use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use foctet_core::{frame::{Frame, OperationId, StreamId}, node::{ConnectionId, NodeAddr, NodeId}};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use crate::{connection::{quic::{QuicConnection, QuicSocket}, tcp::{TcpSocket, TlsTcpStream}, FoctetRecvStream, FoctetSendStream, FoctetStream, NetworkStream, RecvStream, SendStream, Session}, message::{AckMessage, ActorMessage, ControlCommand, ReceiveType, SessionCommand, TransferPayload}, socket::{ConnectionInfo, ConnectionType}};

use super::Socket;

async fn handle_recv_stream(node_id: NodeId, recv_stream: Arc<Mutex<RecvStream>>, incoming_message_tx: mpsc::Sender<ActorMessage>) {
    let mut recv_stream = recv_stream.lock().await;
    loop {
        match recv_stream.receive_frame().await {
            Ok(frame) => {
                tracing::info!("Received frame: {:?}", frame);
                let message = ActorMessage::ReceivedFrame { src_node_id: node_id.clone(), frame: frame };
                match incoming_message_tx.send(message).await {
                    Ok(_) => {
                        tracing::info!("Frame sent to actor");
                    }
                    Err(e) => {
                        tracing::error!("Error sending frame to actor: {:?}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error receiving frame: {:?}", e);
                break;
            }
        }
    }
}

/// Process incoming QUIC stream
async fn run_quic_acceptor(
    mut quic_connection: crate::connection::quic::QuicConnection,
    node_id: NodeId,
    sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    incoming_message_tx: mpsc::Sender<ActorMessage>
) -> Result<()> {
    loop {
        let node_id = node_id.clone();
        match quic_connection.accept_stream().await {
            Ok(stream) => {
                let stream_id = stream.stream_id();
                if let Some(session) = sessions.write().await.get_mut(&node_id) {
                    session.add_stream(stream_id, NetworkStream::Quic(stream)).await;
                    tracing::info!("Accepted QUIC stream: {:?} for node {:?}", stream_id, node_id);
                    match session.get_recv_stream(&stream_id).await {
                        Some(recv_stream) => {
                            tokio::spawn(handle_recv_stream(node_id, recv_stream, incoming_message_tx.clone()));
                        }
                        None => {
                            tracing::error!("Failed to get receive stream for stream: {:?}", stream_id);
                        }
                    }
                } else {
                    tracing::warn!("Session for node {:?} not found while accepting stream", node_id);
                }
            }
            Err(e) => {
                tracing::error!("Error accepting QUIC stream for node {:?}: {:?}", node_id, e);
                break;
            }
        }
    }
    Ok(())
}

async fn run_quic_listener(sessions: Arc<RwLock<HashMap<NodeId, Session>>>, quic_socket: QuicSocket, cancel_token: CancellationToken, incoming_message_tx: mpsc::Sender<ActorMessage>) {
    let (quic_conn_tx, mut quic_conn_rx) = mpsc::channel::<QuicConnection>(100);
    let mut quic_server_socket = quic_socket;
    let cloned_cancel_token = cancel_token.clone();
    tokio::spawn(async move {
        if let Err(e) = quic_server_socket.listen(quic_conn_tx, cloned_cancel_token).await {
            tracing::error!("QUIC listener error: {:?}", e);
        }
    });

    loop {
        tokio::select! {
            // Accept new connection
            Some(quic_connection) = quic_conn_rx.recv() => {
                let node_id = quic_connection.node_id.clone();
                let node_addr = NodeAddr::new(node_id.clone());
                let session = Session::new(
                    ConnectionId::new(),
                    node_addr,
                );

                // Register session
                sessions.write().await.insert(node_id.clone(), session);

                // Accept QUIC stream
                let sessions_clone = sessions.clone();
                let incoming_message_tx_clone = incoming_message_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = run_quic_acceptor(quic_connection, node_id, sessions_clone, incoming_message_tx_clone).await {
                        tracing::error!("Failed to process QUIC stream: {:?}", e);
                    }
                });
            }
            // Check for cancellation
            _ = cancel_token.cancelled() => {
                tracing::info!("QUIC listener cancelled");
                break;
            }
        }
    }
}

async fn run_tcp_listener(sessions: Arc<RwLock<HashMap<NodeId, Session>>>, tcp_socket: TcpSocket, cancel_token: CancellationToken, incoming_message_tx: mpsc::Sender<ActorMessage>) {
    let (tcp_conn_tx, mut tcp_conn_rx) = mpsc::channel::<TlsTcpStream>(100);
    let mut tcp_server_socket = tcp_socket;
    let cloned_cancel_token = cancel_token.clone();
    tokio::spawn(async move {
        if let Err(e) = tcp_server_socket.listen(tcp_conn_tx, cloned_cancel_token).await {
            tracing::error!("TCP listener error: {:?}", e);
        }
    });

    loop {
        tokio::select! {
            // Accept new connection
            Some(tls_tcp_stream) = tcp_conn_rx.recv() => {
                let stream_id = tls_tcp_stream.stream_id();
                let node_id = tls_tcp_stream.node_id.clone();
                // Check if session exists
                // if not, create a new session
                // if exists, add stream to session
                match sessions.read().await.get(&node_id) {
                    Some(session) => {
                        session.add_stream(stream_id, NetworkStream::Tcp(tls_tcp_stream)).await;
                    }
                    None => {
                        let node_addr = NodeAddr::new(node_id.clone());
                        let session = Session::new(ConnectionId::new(), node_addr);
                        session.add_stream(stream_id, NetworkStream::Tcp(tls_tcp_stream)).await;
                        sessions.write().await.insert(node_id.clone(), session);
                    }
                }
                match sessions.read().await.get(&node_id) {
                    Some(session) => {
                        match session.get_recv_stream(&stream_id).await {
                            Some(recv_stream) => {
                                tokio::spawn(handle_recv_stream(node_id, recv_stream, incoming_message_tx.clone()));
                            }
                            None => {
                                tracing::error!("Failed to get receive stream for stream: {:?}", stream_id);
                            }
                        }
                    }
                    None => {
                        tracing::warn!("Session for node {:?} not found while accepting stream", node_id);
                    }
                }
            }
            // Check for cancellation
            _ = cancel_token.cancelled() => {
                tracing::info!("TCP listener cancelled");
                break;
            }
        }
    }
}

pub struct SocketActor {
    /// The socket reference
    pub socket: Arc<RwLock<Socket>>,
    /// Sender for actor messages
    pub msg_sender: mpsc::Sender<ActorMessage>,
    /// Response senders for operations
    pub response_senders: Arc<Mutex<HashMap<OperationId, oneshot::Sender<AckMessage>>>>,
    /// Remote nodes
    pub remote_nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    /// Sessions for connections. Key: Remote NodeId, Value: Session between Local and Remote Node
    pub sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    /// Cancellation token for actor
    pub cancel_token: CancellationToken,
    /// QUIC socket
    quic_socket: QuicSocket,
    /// TCP socket
    tcp_socket: TcpSocket,
}

impl SocketActor {
    pub async fn new(socket: Arc<RwLock<Socket>>, msg_sender: mpsc::Sender<ActorMessage>, cancel_token: CancellationToken) -> Result<Self> {
        let sock = socket.read().await;
        let quic_socket = QuicSocket::new(sock.node_addr.node_id.clone(), sock.config.clone())?;
        let tcp_socket = TcpSocket::new(sock.node_addr.node_id.clone(), sock.config.clone())?;
        drop(sock);
        Ok(Self {
            socket,
            msg_sender,
            response_senders: Arc::new(Mutex::new(HashMap::new())),
            remote_nodes: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cancel_token: cancel_token,
            quic_socket,
            tcp_socket,
        })
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub async fn run(mut self, outgoing_message_rx: mpsc::Receiver<ActorMessage>, incoming_message_tx: mpsc::Sender<ActorMessage>) -> Result<()> {
        // Run QUIC listener
        let quic_socket = self.quic_socket.clone();
        let quic_sessions = self.sessions.clone();
        let quic_cancel_token = self.cancel_token.clone();
        let incoming_message_tx_clone = incoming_message_tx.clone();
        tokio::spawn(async move {
            run_quic_listener(quic_sessions, quic_socket, quic_cancel_token, incoming_message_tx_clone).await;
        });
        // Run TCP listener
        let tcp_socket = self.tcp_socket.clone();
        let tcp_sessions = self.sessions.clone();
        let tcp_cancel_token = self.cancel_token.clone();
        let incoming_message_tx_clone = incoming_message_tx.clone();
        tokio::spawn(async move {
            run_tcp_listener(tcp_sessions, tcp_socket, tcp_cancel_token, incoming_message_tx_clone).await;
        });
        // Run actor message handler
        self.run_message_handler(outgoing_message_rx).await?;
        let _ = self.msg_sender.send(ActorMessage::Ack(AckMessage::ShutdownComplete)).await;
        Ok(())
    }

    pub async fn run_message_handler(&mut self, mut receiver: mpsc::Receiver<ActorMessage>) -> Result<()> {
        loop {
            tokio::select! {
                Some(message) = receiver.recv() => {
                    self.handle_actor_message(message).await?;
                }
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("Actor cancelled");
                    break;
                }
            }
        }
        let _ = self.msg_sender.send(ActorMessage::Ack(AckMessage::ShutdownComplete)).await;
        Ok(())
    }

    async fn handle_actor_message(&mut self, message: ActorMessage) -> Result<()> {
        match message {
            ActorMessage::SessionManagement(session_command) => {
                self.handle_session_management(session_command).await
            }
            ActorMessage::DataTransfer { operation_id, target_node, payload } => {
                self.handle_data_transfer(operation_id, target_node, payload).await
            }
            ActorMessage::Control(control_command) => {
                self.handle_control_command(control_command).await
            }
            _ => {
                tracing::warn!("Unsupported message");
                Ok(())
            }
        }
    }

    async fn handle_session_management(&mut self, session_command: SessionCommand) -> Result<()> {
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
                        let msg = ActorMessage::Ack(AckMessage::Connected(conn_info));
                        self.msg_sender.send(msg).await?;
                    },
                    Err(e) => {
                        let err_msg = ActorMessage::Ack(AckMessage::Error(e));
                        self.msg_sender.send(err_msg).await?;
                    }
                }
            }
            SessionCommand::Disconnect(node_id) => {
                tracing::info!("Disconnecting from node: {:?}", node_id);
                self.disconnect(node_id.clone()).await;
                self.msg_sender.send(ActorMessage::Ack(AckMessage::Disconnected(node_id))).await?;
            }
            _ => {
                tracing::warn!("Unsupported session command");
            }
        }
        Ok(())
    }

    async fn handle_data_transfer(&mut self, operation_id: OperationId, target_node: NodeId, payload: TransferPayload) -> Result<()> {
        match payload {
            TransferPayload::Frame(frame) => {
                tracing::info!("Send frame to {:?}", target_node);
                match self.send_frame(target_node.clone(), frame).await {
                    Ok(_) => {
                        tracing::info!("Frame sent successfully");
                        let msg = ActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                        self.msg_sender.send(msg).await?;
                    }
                    Err(e) => {
                        tracing::error!("Error sending frame: {:?}", e);
                        let err_msg = ActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                        self.msg_sender.send(err_msg).await?;
                    }
                }
            }
            TransferPayload::File(file_path) => {
                tracing::info!("Send file to {:?}", target_node);
                match self.send_file(target_node.clone(), file_path).await {
                    Ok(_) => {
                        tracing::info!("File sent successfully");
                        let msg = ActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                        self.msg_sender.send(msg).await?;
                    }
                    Err(e) => {
                        tracing::error!("Error sending file: {:?}", e);
                        let err_msg = ActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                        self.msg_sender.send(err_msg).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_control_command(&mut self, control_command: ControlCommand) -> Result<()> {
        match control_command {
            ControlCommand::Shutdown => {
                tracing::info!("Shutting down actor");
                self.shutdown().await;
                Ok(())
            }
        }
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
                let connection_id = ConnectionId::new();
                let socket_addr = connection.remote_address();
                let session = Session::new_with_quic_connection(connection_id, node_addr.clone(), connection);
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
                let connection_id = ConnectionId::new();
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
        self.close_sessions().await;
        let _ = self.cancel_token.cancel();
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