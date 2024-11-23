use std::{collections::HashMap, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use foctet_core::{frame::{Frame, OperationId, StreamId}, node::{ConnectionId, NodeAddr, NodeId}};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use crate::{connection::{quic::QuicSocket, tcp::TcpSocket, FoctetStream, NetworkStream, Session}, message::{AckMessage, ActorMessage, ControlCommand, ReceiveType, SessionCommand, TransferPayload}};

use super::Socket;

pub struct SocketActor {
    /// The socket reference
    pub socket: Arc<Socket>,
    /// Sender for actor messages
    pub msg_sender: mpsc::Sender<ActorMessage>,
    /// Remote nodes
    pub remote_nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    /// Sessions for connections. Key: Remote NodeId, Value: Session between Local and Remote Node
    pub sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    /// Cancellation token for actor
    pub cancel_token: CancellationToken,
    /// QUIC socket
    pub quic_socket: QuicSocket,
    /// TCP socket
    pub tcp_socket: TcpSocket,
}

impl SocketActor {
    pub fn new(socket: Arc<Socket>, msg_sender: mpsc::Sender<ActorMessage>, cancel_token: CancellationToken) -> Result<Self> {
        let quic_socket = QuicSocket::new(socket.node_addr.node_id.clone(), socket.config.clone())?;
        let tcp_socket = TcpSocket::new(socket.node_addr.node_id.clone(), socket.config.clone())?;
        Ok(Self {
            socket,
            msg_sender,
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

    pub async fn run(mut self, mut receiver: mpsc::Receiver<ActorMessage>) {
        loop {
            tokio::select! {
                // Receive messages from the ActorMessage channel
                Some(message) = receiver.recv() => {
                    match message {
                        ActorMessage::SessionManagement(session_command) => {
                            match session_command {
                                SessionCommand::Connect(node_addr) => {
                                    tracing::info!("Connecting to node: {:?}", node_addr);
                                    match self.connect(node_addr.clone()).await {
                                        Ok(_) => {
                                            let msg = ActorMessage::Ack(AckMessage::Connected { node_id: node_addr.node_id });
                                            let _ = self.msg_sender.send(msg).await;
                                        },
                                        Err(e) => {
                                            let err_msg = ActorMessage::Ack(AckMessage::Failure(e));
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                                SessionCommand::Disconnect(node_id) => {
                                    tracing::info!("Disconnecting from node: {:?}", node_id);
                                    self.disconnect(node_id.clone()).await;
                                    let _ = self.msg_sender.send(ActorMessage::Ack(AckMessage::Disconnected(node_id))).await;
                                }
                                _ => {
                                    tracing::warn!("Unsupported session command");
                                }
                            }
                        }
                        ActorMessage::DataTransfer { operation_id, target_node, payload } => {
                            match payload {
                                TransferPayload::Frame(frame) => {
                                    tracing::info!("Send frame to {:?}", target_node);
                                    match self.send_frame(target_node.clone(), frame).await {
                                        Ok(_) => {
                                            tracing::info!("Frame sent successfully");
                                            let msg = ActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error sending frame: {:?}", e);
                                            let err_msg = ActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                                TransferPayload::File(file_path) => {
                                    tracing::info!("Send file to {:?}", target_node);
                                    match self.send_file(target_node.clone(), file_path).await {
                                        Ok(_) => {
                                            tracing::info!("File sent successfully");
                                            let msg = ActorMessage::Ack(AckMessage::TransferComplete { operation_id, node_id: target_node });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error sending file: {:?}", e);
                                            let err_msg = ActorMessage::Ack(AckMessage::TransferError { operation_id, node_id: target_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                            }
                        }
                        ActorMessage::DataReceive {source_node, receive_type} => {
                            match receive_type {
                                ReceiveType::Frame => {
                                    tracing::info!("Receive frame from {:?}", source_node);
                                    match self.receive_frame(source_node.clone()).await {
                                        Ok(frame) => {
                                            tracing::info!("Frame received successfully");
                                            let msg = ActorMessage::Ack(AckMessage::FrameReceived { node_id: source_node, frame });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error receiving frame: {:?}", e);
                                            let err_msg = ActorMessage::Ack(AckMessage::ReceiveError { node_id: source_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                                ReceiveType::File(path) => {
                                    tracing::info!("Receive file from {:?}", source_node);
                                    match self.receive_file(source_node.clone(), &path).await {
                                        Ok(byte_size) => {
                                            tracing::info!("File received successfully");
                                            let msg = ActorMessage::Ack(AckMessage::FileReceived { node_id: source_node, path, byte_size });
                                            let _ = self.msg_sender.send(msg).await;
                                        }
                                        Err(e) => {
                                            tracing::error!("Error receiving file: {:?}", e);
                                            let err_msg = ActorMessage::Ack(AckMessage::ReceiveError { node_id: source_node, error: e });
                                            let _ = self.msg_sender.send(err_msg).await;
                                        }
                                    }
                                }
                            }
                        },
                        ActorMessage::Control(control_command) => {
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
        let _ = self.msg_sender.send(ActorMessage::Ack(AckMessage::ShutdownComplete)).await;
    }

    async fn connect(&mut self, node_addr: NodeAddr) -> Result<()> {
        tracing::info!("Attempting to connect to {:?}", node_addr);
        // 1. Try QUIC connection
        match self.quic_socket.connect_node(node_addr.clone()).await {
            Ok(mut connection) => {
                tracing::info!("Successfully connected to {:?} via QUIC", node_addr.node_id);
                let stream = connection.open_stream().await?;
                // Create session
                let connection_id = ConnectionId::new();
                let session = Session::new_with_quic_connection(connection_id, node_addr.clone(), connection);
                let stream_id = stream.stream_id();
                let stream_arc = Arc::new(Mutex::new(NetworkStream::Quic(stream)));
                session.add_stream(stream_id, stream_arc).await;
                self.sessions.write().await.insert(node_addr.node_id.clone(), session);
                return Ok(());
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
                let stream_arc = Arc::new(Mutex::new(NetworkStream::Tcp(stream)));
                let connection_id = ConnectionId::new();
                let session = Session::new(connection_id, node_addr.clone());
                session.add_stream(stream_id, stream_arc).await;
                self.sessions.write().await.insert(node_addr.node_id.clone(), session);
                return Ok(());
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

    pub async fn get_available_stream(&self, node_id: NodeId) -> Option<Arc<Mutex<NetworkStream>>> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&node_id)?;
        session.get_available_stream().await
    }

    pub async fn open_stream(&mut self, node_id: NodeId) -> Result<Arc<Mutex<NetworkStream>>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&node_id).ok_or(anyhow::anyhow!("Session not found"))?;
        if let Some(quic_connection) = &mut session.quic_connection {
            let stream = quic_connection.open_stream().await?;
            let stream_id = stream.stream_id();
            let stream_arc = Arc::new(Mutex::new(NetworkStream::Quic(stream)));
            session.add_stream(stream_id, Arc::clone(&stream_arc)).await;
            return Ok(stream_arc);
        } else {
            // Open a new TCP stream
            let stream = self.tcp_socket.connect_node(session.node_addr.clone()).await?;
            let stream_id = stream.stream_id();
            let stream_arc = Arc::new(Mutex::new(NetworkStream::Tcp(stream)));
            session.add_stream(stream_id, Arc::clone(&stream_arc)).await;
            return Ok(stream_arc);
        }
    }

    pub async fn get_stream(&mut self, node_id: NodeId) -> Result<Arc<Mutex<NetworkStream>>> {
        match self.get_available_stream(node_id.clone()).await {
            Some(stream) => Ok(stream),
            None => self.open_stream(node_id).await,
        }
    }

    pub async fn find_stream(
        &self,
        node_id: NodeId,
        stream_id: StreamId,
    ) -> Option<Arc<Mutex<NetworkStream>>> {
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
            Ok(stream) => {
                let mut stream = stream.lock().await;
                stream.send_frame(frame).await
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
            Ok(stream) => {
                let mut stream = stream.lock().await;
                stream.receive_frame().await
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
            Ok(stream) => {
                let mut stream = stream.lock().await;
                stream.send_file(&path).await
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
            Ok(stream) => {
                let mut stream = stream.lock().await;
                stream.receive_file(path).await
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
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // 毎分チェック
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