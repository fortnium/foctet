use foctet::{core::{frame::{Frame, FrameType, Payload}, node::{NodeId, NodePair}}, net::{config::TransportProtocol, connection::{FoctetStream, NetworkStream}, endpoint::{Endpoint, EndpointHandle, Listener}}};
use tokio::sync::mpsc;
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use anyhow::Result;

use crate::{client::ClientManager, config::{RelayConfig, ServerConfig}, message::{RelayedConnection, ServerMessage}};

#[derive(Debug)]
pub struct RelayServerActor {
    pub server_channel_rx: mpsc::Receiver<ServerMessage>,
    pub client_manager: ClientManager,
}

impl RelayServerActor {
    pub fn new(server_channel_tx: mpsc::Sender<ServerMessage>, server_channel_rx: mpsc::Receiver<ServerMessage>) -> Self {
        Self {
            server_channel_rx,
            client_manager: ClientManager::new(server_channel_tx),
        }
    }
    /// Start the server actor loop.
    pub async fn run(mut self, cancel: CancellationToken) -> Result<()> {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Actor loop cancelled, closing loop");
                    self.client_manager.shutdown().await;
                    return Ok(());
                }
                msg = self.server_channel_rx.recv() => match msg {
                    Some(msg) => {
                        self.handle_message(msg).await;
                    }
                    None => {
                        tracing::warn!("Actor error: receiver gone, shutting down actor loop");
                        self.client_manager.shutdown().await;
                        anyhow::bail!("Actor error, closed client connections, and shutting down actor loop");
                    }
                }
            }
        }
    }
    async fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::SendPacket(packet) => {
                // Send the packet to DST node
                self.client_manager.send_packet(packet).await;
            }
            ServerMessage::AddClient(relayed_connection) => {
                // Add a new client connection
                let node_pair = NodePair::new(relayed_connection.src, relayed_connection.dst);
                self.client_manager.register(node_pair, relayed_connection.stream).await;
            }
            ServerMessage::RemoveClient(node_pair) => {
                // Remove the client connection
                self.client_manager.unregister(node_pair).await;
            }
        }
    }
}

#[derive(Debug)]
pub struct RelayServerHandle {
    cancel: CancellationToken,
}

impl RelayServerHandle {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
        }
    }
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

#[derive(Debug)]
pub struct RelayServer {
    pub config: RelayConfig,
    /// Server message channel to send messages to the server actor.
    pub server_channel_tx: mpsc::Sender<ServerMessage>,
    /// Server actor loop handler
    actor_loop_handler: AbortOnDropHandle<Result<()>>,
    /// Cancellation token to stop the server actor loop and QUIC/TCP listeners.
    cancel: CancellationToken,
}

impl RelayServer {
    pub fn spawn(config: RelayConfig) -> Result<Self> {
        let (server_channel_tx, server_channel_rx) = mpsc::channel(100);
        let actor = RelayServerActor::new(server_channel_tx.clone(), server_channel_rx);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let actor_loop_handler = AbortOnDropHandle::new(tokio::spawn(async move {
            actor.run(cancel_clone).await
        }));
        Ok(Self {
            config,
            server_channel_tx,
            actor_loop_handler,
            cancel,
        })
    }
    pub fn server_channel_sender(&self) -> mpsc::Sender<ServerMessage> {
        self.server_channel_tx.clone()
    }
    pub fn handle(&self) -> RelayServerHandle {
        RelayServerHandle::new(self.cancel.clone())
    }
    /// Closes the server and waits for the connections to disconnect.
    pub async fn close(self) {
        self.cancel.cancel();
        match self.actor_loop_handler.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("error shutting down server: {e:#}"),
            Err(e) => tracing::warn!("error waiting for the server process to close: {e:?}"),
        }
    }
}

#[derive(Debug)]
pub struct Server {
    endpoint_handle: EndpointHandle,
    relay_handle: RelayServerHandle,
    server_handle: AbortOnDropHandle<Result<()>>,
}

impl Server {
    pub async fn spawn(config: ServerConfig) -> Result<Self> {
        // Create a QUIC/TCP server endpoint
        let mut endpoint = Endpoint::builder()
            .with_node_addr(config.node_addr())
            .with_config(config.endpoint_config.clone())
            .build().await?;
        // Start listening for incoming connections
        let listener = endpoint.listen().await?;
        let endpoint_handle = endpoint.handle();
        // Create a relay server
        let relay_server = RelayServer::spawn(config.relay_config())?;
        let relay_handle = relay_server.handle();

        let server_handle = AbortOnDropHandle::new(tokio::spawn(run_server_task(listener, relay_server)));
        
        Ok(Self {
            endpoint_handle,
            relay_handle,
            server_handle,
        })
    }
    pub fn handle(&mut self) -> &mut AbortOnDropHandle<Result<()>> {
        &mut self.server_handle
    }
    pub async fn shutdown(self) -> Result<()> {
        self.relay_handle.shutdown();
        self.endpoint_handle.shutdown().await;
        self.server_handle.await?
    }
}

async fn run_server_task(mut listener: Listener, relay_server: RelayServer) -> Result<()> {
    // Handle incoming streams
    let sender = relay_server.server_channel_sender();
    while let Some(stream) = listener.accept().await {
        let sender = sender.clone();
        tokio::spawn(async move {
            match stream.transport_protocol() {
                TransportProtocol::Quic => {
                    tracing::info!("New QUIC connection from: {}", stream.remote_address());
                }
                TransportProtocol::Tcp => {
                    tracing::info!("New TCP connection from: {}", stream.remote_address());
                }
                _ => {
                    tracing::info!("New connection from: {}", stream.remote_address());
                }
            }
            handle_stream(stream, sender).await;
        });
    }
    Ok(())
}

async fn handle_stream(mut stream: NetworkStream, sender: mpsc::Sender<ServerMessage>) {
    tracing::info!("Handling stream from: {}", stream.remote_address());
    // Wait for handshake
    // if handshake is successful, add the client to the relay server
    let mut handshaked = false;
    let mut src_node_id = NodeId::zero();
    let mut dst_node_id = NodeId::zero();
    loop {
        match stream.receive_frame().await {
            Ok(frame) => {
                tracing::info!("Received frame: {:?}", frame);
                match frame.frame_type {
                    FrameType::Connect => {
                        if let Some(payload) = frame.payload {
                            match payload {
                                Payload::HandshakeRelay(handshake) => {
                                    src_node_id = handshake.src_node_id;
                                    dst_node_id = handshake.dst_node_id;
                                }
                                _ => {
                                    tracing::warn!("Invalid payload for handshake: {:?}", payload);
                                    continue;
                                }
                            }
                        }
                        let frame: Frame = Frame::builder()
                            .with_fin(true)
                            .with_frame_type(FrameType::Connected)
                            .as_response()
                            .build();
                        match stream.send_frame(frame).await {
                            Ok(_) => {
                                handshaked = true;
                                tracing::info!("Sent connected frame back");
                            }
                            Err(e) => {
                                tracing::info!("Failed to send frame back: {:?}", e);
                            }
                        }
                    }
                    _ => {
                        tracing::info!("Unknown frame type: {:?}", frame);
                    }
                }
            }
            Err(e) => {
                tracing::info!("Error receiving frame: {:?}", e);
                break;
            }
        }
        if handshaked {
            let relayed_connection = RelayedConnection {
                src: src_node_id,
                dst: dst_node_id,
                stream,
            };
            let _ = sender.send(ServerMessage::AddClient(relayed_connection)).await;
            break;
        }
    }
}
