use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use foctet_core::{frame::{Frame, OperationId, StreamId}, node::{SessionId, NodeAddr, NodeId}};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use crate::{config::EndpointConfig, connection::{quic::{QuicConnection, QuicSocket}, tcp::{TcpSocket, TlsTcpStream}, ConnectionInfo, ConnectionType, FoctetRecvStream, FoctetSendStream, FoctetStream, NetworkStream, RecvStream, SendStream, Session}, message::{AckMessage, ControlCommand, RelayActorMessage, SessionCommand, TransferPayload}};

pub struct RelayActor {
    pub node_addr: NodeAddr,
    pub config: EndpointConfig,
    /// Sender for actor messages
    pub msg_sender: mpsc::Sender<RelayActorMessage>,
    /// Destination nodes
    pub destination_nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    /// Sessions for connections. Key: Destination NodeId, Value: Session between Local and Relay Node
    pub sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    /// Cancellation token for actor
    pub cancel_token: CancellationToken,
    /// QUIC socket
    pub quic_socket: QuicSocket,
    /// TCP socket
    pub tcp_socket: TcpSocket,
}

impl RelayActor {
    pub async fn new(node_addr: NodeAddr, config: EndpointConfig, msg_sender: mpsc::Sender<RelayActorMessage>, cancel_token: CancellationToken) -> Result<Self> {
        let quic_socket = QuicSocket::new(node_addr.node_id.clone(), config.clone())?;
        let tcp_socket = TcpSocket::new(node_addr.node_id.clone(), config.clone())?;
        Ok(Self {
            node_addr,
            config,
            msg_sender,
            destination_nodes: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cancel_token,
            quic_socket,
            tcp_socket,
        })
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<RelayActorMessage>) {
        // Run QUIC listener
        let quic_sessions = self.sessions.clone();
        let quic_socket = self.quic_socket.clone();
        let quic_cancel_token = self.cancel_token.clone();
        tokio::spawn(async move {
            Self::run_quic_listener(quic_sessions, quic_socket, quic_cancel_token).await;
        });
        // Run TCP listener
        let tcp_sessions = self.sessions.clone();
        let tcp_socket = self.tcp_socket.clone();
        let tcp_cancel_token = self.cancel_token.clone();
        tokio::spawn(async move {
            Self::run_tcp_listener(tcp_sessions, tcp_socket, tcp_cancel_token).await;
        });
        
        loop {
            tokio::select! {
                // Receive messages from the ActorMessage channel
                Some(message) = receiver.recv() => {
                    match message {
                        RelayActorMessage::SessionManagement(session_command) => {
                            match session_command {
                                SessionCommand::Connect(node_addr) => {
                                    tracing::info!("Connecting to node: {:?}", node_addr);
                                    match self.connect(node_addr.clone()).await {
                                        Ok(socket_addr) => {
                                            let conn_info = ConnectionInfo {
                                                node_id: node_addr.node_id.clone(),
                                                socket_addr: socket_addr,
                                                connection_type: ConnectionType::Direct,
                                            };
                                            let msg = RelayActorMessage::Ack(AckMessage::Connected(conn_info));
                                            let _ = self.msg_sender.send(msg).await;
                                        },
                                        Err(e) => {
                                            let err_msg = RelayActorMessage::Ack(AckMessage::Error(e));
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                                SessionCommand::Disconnect(node_id) => {
                                    tracing::info!("Disconnecting from node: {:?}", node_id);
                                    self.disconnect(node_id.clone()).await;
                                    let _ = self.msg_sender.send(RelayActorMessage::Ack(AckMessage::Disconnected(node_id))).await;
                                }
                                _ => {
                                    tracing::warn!("Unsupported session command");
                                }
                            }
                        }
                        RelayActorMessage::DataTransfer { operation_id, target_node, payload } => {
                            match payload {
                                TransferPayload::Frame(frame) => {
                                    tracing::info!("Send frame to {:?}", target_node);
                                    match self.send_frame(target_node.clone(), frame).await {
                                        Ok(_) => {
                                            tracing::info!("Frame sent successfully");
                                            let msg = RelayActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error sending frame: {:?}", e);
                                            let err_msg = RelayActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                                TransferPayload::File(file_path) => {
                                    tracing::info!("Send file to {:?}", target_node);
                                    match self.send_file(target_node.clone(), file_path).await {
                                        Ok(_) => {
                                            tracing::info!("File sent successfully");
                                            let msg = RelayActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error sending file: {:?}", e);
                                            let err_msg = RelayActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                            }
                        }
                        RelayActorMessage::Control(control_command) => {
                            match control_command {
                                ControlCommand::Shutdown => {
                                    tracing::info!("Shutting down actor");
                                    self.shutdown().await;
                                    break;
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("Unsupported message");
                        }
                    }
                }
                // Receive cancellation signal
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("Actor cancelled");
                    break;
                }
            }
        }
        let _ = self.msg_sender.send(RelayActorMessage::Ack(AckMessage::ShutdownComplete)).await;
    }

    async fn connect(&mut self, node_addr: NodeAddr) -> Result<SocketAddr> {
        tracing::info!("Attempting to connect to {:?}", node_addr);
        // 1. Try QUIC connection
        match self.quic_socket.connect_node(node_addr.clone()).await {
            Ok(mut connection) => {
                tracing::info!("Successfully connected to {:?} via QUIC", node_addr.node_id);
                let stream = connection.open_stream().await?;
                let stream_id = stream.stream_id();
                // Create session
                let session_id = SessionId::new();
                let socket_addr = connection.remote_address();
                let session = Session::new_with_quic_connection(session_id, node_addr.clone(), connection);
                session.add_stream(stream_id, NetworkStream::Quic(stream)).await;
                self.sessions.write().await.insert(node_addr.node_id.clone(), session);
                return Ok(socket_addr);
            }
            Err(e) => {
                tracing::warn!("Failed to connect to {:?} via QUIC: {:?}", node_addr.node_id, e);
                tracing::info!("Attempting to connect to {:?} via TCP", node_addr.node_id);
            }
        }
        // 2. Try TCP connection
        match self.tcp_socket.connect_node(node_addr.clone()).await {
            Ok(stream) => {
                tracing::info!("Successfully connected to {:?} via TCP", node_addr.node_id);
                let stream_id = stream.stream_id();
                let socket_addr = stream.remote_address();
                // Create session
                let connection_id = SessionId::new();
                let session = Session::new(connection_id, node_addr.clone());
                session.add_stream(stream_id, NetworkStream::Tcp(stream)).await;
                self.sessions.write().await.insert(node_addr.node_id.clone(), session);
                return Ok(socket_addr);
            }
            Err(e) => {
                tracing::error!("Failed to connect to {:?} via TCP: {:?}", node_addr.node_id, e);
            }
        }
        Err(anyhow::anyhow!("Failed to connect to {:?}", node_addr))
    }

    async fn disconnect(&self, node_id: NodeId) {
        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(&node_id) {
            session.close().await;
        }
    }

    async fn run_quic_listener(sessions: Arc<RwLock<HashMap<NodeId, Session>>>, quic_socket: QuicSocket, cancel_token: CancellationToken) {
        let (quic_conn_tx, mut quic_conn_rx) = mpsc::channel::<QuicConnection>(100);
        let mut quic_server_socket = quic_socket;
        let cloned_cancel_token = cancel_token.clone();
        tokio::spawn(async move {
            if let Err(e) = quic_server_socket.listen(quic_conn_tx, cloned_cancel_token).await {
                tracing::error!("QUIC listener error: {:?}", e);
            }
        });
    
        while let Some(mut quic_connection) = quic_conn_rx.recv().await {
            if cancel_token.is_cancelled() {
                break;
            }
            let node_id = quic_connection.node_id.clone();
            let stream = quic_connection.accept_stream().await.unwrap();
            let stream_id = stream.stream_id();
            let node_addr = NodeAddr::new(node_id.clone());
            let session = Session::new_with_quic_connection(SessionId::new(), node_addr, quic_connection);
            session.add_stream(stream_id, NetworkStream::Quic(stream)).await;
            sessions.write().await.insert(node_id, session);
        }
    }
    
    async fn run_tcp_listener(sessions: Arc<RwLock<HashMap<NodeId, Session>>>, tcp_socket: TcpSocket, cancel_token: CancellationToken) {
        let (tcp_conn_tx, mut tcp_conn_rx) = mpsc::channel::<TlsTcpStream>(100);
        let mut tcp_server_socket = tcp_socket;
        let cloned_cancel_token = cancel_token.clone();
        tokio::spawn(async move {
            if let Err(e) = tcp_server_socket.listen(tcp_conn_tx, cloned_cancel_token).await {
                tracing::error!("TCP listener error: {:?}", e);
            }
        });
    
        while let Some(tcp_connection) = tcp_conn_rx.recv().await {
            if cancel_token.is_cancelled() {
                break;
            }
            let node_id = tcp_connection.node_id.clone();
            let stream_id = tcp_connection.stream_id();
            let node_addr = NodeAddr::new(node_id.clone());
            let session = Session::new(SessionId::new(), node_addr);
            session.add_stream(stream_id, NetworkStream::Tcp(tcp_connection)).await;
            sessions.write().await.insert(node_id, session);
        }
    }

    pub async fn get_available_stream(&self, node_id: NodeId) -> Option<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&node_id)?;
        let (send_stream, recv_stream) = session.get_available_stream().await?;
        Some((send_stream, recv_stream))
    }

    pub async fn open_stream(&mut self, node_id: NodeId) -> Result<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&node_id).ok_or(anyhow::anyhow!("Session not found"))?;
        if let Some(quic_connection) = &mut session.quic_connection {
            // Open a new QUIC stream
            let stream = quic_connection.open_stream().await?;
            let stream_id = stream.stream_id();
            session.add_stream(stream_id, NetworkStream::Quic(stream)).await;
            match session.get_stream(&stream_id).await {
                Some(stream) => {
                    return Ok(stream);
                }
                None => {
                    return Err(anyhow::anyhow!("Failed to open stream"));
                }
            }
        } else {
            // Open a new TCP stream
            let stream = self.tcp_socket.connect_node(session.node_addr.clone()).await?;
            let stream_id = stream.stream_id();
            session.add_stream(stream_id, NetworkStream::Tcp(stream)).await;
            match session.get_stream(&stream_id).await {
                Some(stream) => {
                    return Ok(stream);
                }
                None => {
                    return Err(anyhow::anyhow!("Failed to open stream"));
                }
            }
        }
    }

    pub async fn get_stream(&mut self, node_id: NodeId) -> Result<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        match self.get_available_stream(node_id.clone()).await {
            Some(stream) => {
                return Ok(stream);
            }
            None => {
                self.open_stream(node_id.clone()).await
            }
        }
    }

    pub async fn find_stream(
        &self,
        node_id: NodeId,
        stream_id: StreamId,
    ) -> Option<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&node_id)?;
        let stream = session.get_stream(&stream_id).await?;
        Some(stream)
    }

    pub async fn send_frame(
        &mut self,
        node_id: NodeId,
        frame: Frame,
    ) -> Result<OperationId> {
        match self.get_stream(node_id).await {
            Ok((send_stream, _recv_stream)) => {
                let mut send_stream = send_stream.lock().await;
                send_stream.send_frame(frame).await
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    pub async fn receive_frame(
        &mut self,
        node_id: NodeId,
    ) -> Result<Frame> {
        match self.get_stream(node_id).await {
            Ok((_send_stream, recv_stream)) => {
                let mut recv_stream = recv_stream.lock().await;
                recv_stream.receive_frame().await
            }
            Err(e) => {
                Err(e)
            }
        }
    }
    
    pub async fn send_file(
        &mut self,
        node_id: NodeId,
        path: PathBuf,
    ) -> Result<OperationId> {
        match self.get_stream(node_id).await {
            Ok((send_stream, _recv_stream)) => {
                let mut send_stream = send_stream.lock().await;
                send_stream.send_file(&path).await
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    pub async fn receive_file(
        &mut self,
        node_id: NodeId,
        path: &std::path::Path,
    ) -> Result<u64> {
        match self.get_stream(node_id).await {
            Ok((_send_stream, recv_stream)) => {
                let mut recv_stream = recv_stream.lock().await;
                recv_stream.receive_file(path).await
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    pub async fn shutdown(&mut self) {
        let _ = self.cancel_token.cancel();
        self.close_sessions().await;
    }
    
    pub async fn close_sessions(&mut self) {
        let mut sessions = self.sessions.write().await;
        for (_, session) in sessions.iter_mut() {
            session.close().await;
        }
    }

    pub async fn cleanup_session(&mut self, node_id: NodeId) {
        let sessions = self.sessions.write().await;
        sessions.get(&node_id).map(|session| {
            let _ = session.cleanup_streams();
        });
    }

    pub async fn run_cleanup_task(&self) {
        // TODO: Get cleanup interval from config
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            self.cleanup_old_sessions().await;
        }
    }

    pub async fn cleanup_old_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();
        let mut sessions_to_remove = Vec::new();
        for (node_id, session) in sessions.iter() {
            let last_accessed = session.last_accessed.lock().await;
            if now.duration_since(*last_accessed) >= Duration::from_secs(300) {
                sessions_to_remove.push(node_id.clone());
            }
        }
        for node_id in sessions_to_remove {
            sessions.remove(&node_id);
        }
    }
}
