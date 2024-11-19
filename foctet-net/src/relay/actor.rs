use std::{collections::HashMap, path::PathBuf, sync::Arc, time::{Duration, Instant}};

use foctet_core::{frame::{Frame, OperationId, StreamId}, node::{ConnectionId, NodeAddr, NodeId}};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use crate::{connection::{quic::QuicSocket, tcp::TcpSocket, FoctetStream, NetworkStream, Session}, message::{AckMessage, ActorMessage, ControlCommand, RelayActorMessage, SessionCommand, TransferPayload}};

use crate::socket::Socket;

pub struct RelaySocketActor {
    /// The socket reference
    pub socket: Arc<Socket>,
    /// Sender for actor messages
    pub msg_sender: mpsc::Sender<ActorMessage>,
    /// Destination nodes
    pub destination_nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    /// Sessions for connections. Key: Destination NodeId, Value: Session between Local and Relay Node
    pub relay_sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    /// Cancellation token for actor
    pub cancel_token: CancellationToken,
    /// QUIC socket
    pub quic_socket: QuicSocket,
    /// TCP socket
    pub tcp_socket: TcpSocket,
}

impl RelaySocketActor {
    pub fn new(socket: Arc<Socket>, msg_sender: mpsc::Sender<ActorMessage>) -> Result<Self> {
        let quic_socket = QuicSocket::new(socket.node_addr.node_id.clone(), socket.config.clone())?;
        let tcp_socket = TcpSocket::new(socket.node_addr.node_id.clone(), socket.config.clone())?;
        Ok(Self {
            socket,
            msg_sender,
            destination_nodes: Arc::new(RwLock::new(HashMap::new())),
            relay_sessions: Arc::new(RwLock::new(HashMap::new())),
            cancel_token: CancellationToken::new(),
            quic_socket,
            tcp_socket,
        })
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<RelayActorMessage>) {
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
                                        Ok(_) => {
                                            let msg = ActorMessage::Ack(AckMessage::Connected { node_id: node_addr.node_id});
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
                        RelayActorMessage::DataTransfer { operation_id, target_node, stream_id, payload } => {
                            match payload {
                                TransferPayload::Frame(frame) => {
                                    tracing::info!("Send frame to {:?} on {}", target_node, stream_id);
                                    match self.send_frame(target_node.clone(), stream_id, frame).await {
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
                                    tracing::info!("Send file to {:?} on {}", target_node, stream_id);
                                    match self.send_file(target_node.clone(), stream_id, file_path).await {
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
                let session = Session::new(connection_id, node_addr.clone());
                let stream_id = stream.stream_id();
                let stream_arc = Arc::new(Mutex::new(NetworkStream::Quic(stream)));
                session.add_stream(stream_id, stream_arc).await;
                self.relay_sessions.write().await.insert(node_addr.node_id.clone(), session);
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
                self.relay_sessions.write().await.insert(node_addr.node_id.clone(), session);
                return Ok(());
            }
            Err(e) => {
                tracing::error!("Failed to connect to {:?} via TCP: {:?}", node_addr.node_id, e);
            }
        }
        Err(anyhow::anyhow!("Failed to connect to {:?}", node_addr))
    }

    async fn disconnect(&self, node_id: NodeId) {
        let mut sessions = self.relay_sessions.write().await;
        if let Some(session) = sessions.remove(&node_id) {
            session.close().await;
        }
    }

    pub async fn find_stream(
        &self,
        node_id: NodeId,
        stream_id: StreamId,
    ) -> Option<Arc<Mutex<NetworkStream>>> {
        let sessions = self.relay_sessions.read().await;
        let session = sessions.get(&node_id)?;
        let stream = session.get_stream(&stream_id).await?;
        Some(stream)
    }

    pub async fn send_frame(
        &mut self,
        node_id: NodeId,
        stream_id: StreamId,
        frame: Frame,
    ) -> Result<OperationId> {
        match self.find_stream(node_id, stream_id).await {
            Some(stream) => {
                let mut stream = stream.lock().await;
                stream.send_frame(frame).await
            }
            None => {
                Err(anyhow::anyhow!("Stream not found"))
            }
        }
    }

    pub async fn receive_frame(
        &mut self,
        node_id: NodeId,
        stream_id: StreamId,
    ) -> Result<Frame> {
        match self.find_stream(node_id, stream_id).await {
            Some(stream) => {
                let mut stream = stream.lock().await;
                stream.receive_frame().await
            }
            None => {
                Err(anyhow::anyhow!("Stream not found"))
            }
        }
    }
    
    pub async fn send_file(
        &mut self,
        node_id: NodeId,
        stream_id: StreamId,
        path: PathBuf,
    ) -> Result<OperationId> {
        match self.find_stream(node_id, stream_id).await {
            Some(stream) => {
                let mut stream = stream.lock().await;
                stream.send_file(&path).await
            }
            None => {
                Err(anyhow::anyhow!("Stream not found"))
            }
        }
    }

    pub async fn receive_file(
        &mut self,
        node_id: NodeId,
        stream_id: StreamId,
        path: &std::path::Path,
    ) -> Result<u64> {
        match self.find_stream(node_id, stream_id).await {
            Some(stream) => {
                let mut stream = stream.lock().await;
                stream.receive_file(path).await
            }
            None => {
                Err(anyhow::anyhow!("Stream not found"))
            }
        }
    }

    pub async fn shutdown(&mut self) {
        let _ = self.cancel_token.cancel();
    }
    
    pub async fn cleanup_sessions(&mut self) {
        let mut sessions = self.relay_sessions.write().await;
        sessions.clear();
    }

    pub async fn run_cleanup_task(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // 毎分チェック
        loop {
            interval.tick().await;
            self.cleanup_old_sessions().await;
        }
    }

    pub async fn cleanup_old_sessions(&self) {
        let mut sessions = self.relay_sessions.write().await;
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
