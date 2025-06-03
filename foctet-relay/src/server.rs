use foctet_core::{addr::node::{NodeAddr, RelayAddr}, frame::{Frame, FrameType}, id::NodeId, key::Keypair, proto::relay::{RegisterRequest, RegisterResponse, TunnelId, TunnelRequest, TunnelResponse}, transport::{ListenerId, TransportKind}};
use foctet_net::{config::TransportConfig, device::{get_default_stack_addrs, get_unspecified_stack_addrs, replace_with_actual_addrs}, transport::{connection::{Connection, ConnectionEvent}, quic::transport::QuicTransport, stream::Stream, tcp::transport::TcpTransport, Transport}};
use stackaddr::{segment::protocol::TransportProtocol, StackAddr};
use tokio_util::{sync::CancellationToken};
use std::{collections::{HashMap, HashSet}, sync::{Arc}};
use tokio::{io::AsyncWriteExt, sync::{mpsc, oneshot, RwLock}};
use anyhow::Result;

use crate::tunnel::TunnelStat;

pub enum ServerEvent {
    ClientConnected(NodeId),
    ClientDisconnected(NodeId),
    ClientRegistered(NodeId),
    ClientUnregistered(NodeId),
    TunnelEstablished {
        src_node: NodeId,
        dst_node: NodeId,
    },
    TunnelFailed {
        src_node: NodeId,
        dst_node: NodeId,
        reason: String,
    },
}

pub enum ServerCommand {
    Shutdown,
}

#[derive(Clone)]
pub struct RelayServerState {
    pub nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    pub pending_streams: Arc<RwLock<HashMap<TunnelId, oneshot::Sender<Result<Stream>>>>>,
    pub control_streams: Arc<RwLock<HashMap<NodeId, mpsc::Sender<Frame>>>>,
    pub tunnels: Arc<RwLock<HashMap<TunnelId, TunnelStat>>>,
}

impl RelayServerState {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            pending_streams: Arc::new(RwLock::new(HashMap::new())),
            control_streams: Arc::new(RwLock::new(HashMap::new())),
            tunnels: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

pub struct RelayServerActor {
    config: TransportConfig,
    addrs: HashSet<StackAddr>,
    event_sender: mpsc::Sender<ServerEvent>,
    cmd_receiver: mpsc::Receiver<ServerCommand>,
    state: RelayServerState,
    cancel: CancellationToken,
}

impl RelayServerActor {
    pub async fn run(&mut self) -> Result<()> {
        let mut listerner_id = ListenerId::new(1);
        // Create a channel for endpoint events
        for addr in &self.addrs {
            let config = self.config.clone();
            let mut transport: Transport = match addr.transport() {
                Some(transport) => match transport {
                    TransportProtocol::Quic(_) | TransportProtocol::Udp(_) => {
                        let t = QuicTransport::new(config)?;
                        Transport::Quic(t)
                    },
                    TransportProtocol::TlsTcp(_) | TransportProtocol::Tcp(_) => {
                        let t = TcpTransport::new(config)?;
                        Transport::Tcp(t)
                    },
                    _ => return Err(anyhow::anyhow!("Unsupported transport protocol: {:?}", transport)),
                },
                None => {
                    return Err(anyhow::anyhow!("Invalid transport protocol"));
                }
            };
            // Listen for incoming connections
            let event_sender = self.event_sender.clone();
            let state = self.state.clone();
            let mut listener = transport.listen_on(listerner_id.fetch_add(1), addr.clone()).await?;
            tokio::spawn(async move {
                while let Some(conn_event) = listener.accept().await {
                    match conn_event {
                        ConnectionEvent::Accepted(mut conn) => {
                            tracing::info!("Accepted connection from {}", conn.remote_addr());
                            event_sender.send(ServerEvent::ClientConnected(conn.remote_node_id())).await.unwrap();
                            let event_sender = event_sender.clone();
                            let state = state.clone();
                            tokio::spawn(async move {
                                let _ = RelayServerActor::handle_connection(&mut conn, event_sender, state).await;
                            });
                        }
                        _ => {},
                    }
                }
            });
        }
        // Handle commands
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    tracing::info!("EndpointActor loop cancelled, closing loop");
                    break;
                }
                Some(cmd) = self.cmd_receiver.recv() => {
                    match cmd {
                        ServerCommand::Shutdown => {
                            tracing::info!("RelayServer shutting down");
                            // TODO: Clean up resources, close connections, etc.
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_connection(conn: &mut Connection, event_sender: mpsc::Sender<ServerEvent>, state: RelayServerState) -> Result<()> {
        // Handle the connection
        let remote_addr = conn.remote_addr();
        let node_id = conn.remote_node_id();
        tracing::info!("Handling connection from {}", remote_addr);
        loop {
            // Accept new stream
            tracing::info!("Accepting a new stream...");
            let event_sender = event_sender.clone();
            match conn.accept_stream().await {
                Ok(stream) => {
                    let state = state.clone();
                    tokio::spawn(async move {
                        let event_sender = event_sender.clone();
                        let _ = RelayServerActor::handle_stream(node_id, stream, event_sender, state).await;
                    });
                }
                Err(e) => {
                    tracing::info!("Error accepting stream: {:?}", e);
                    event_sender.send(ServerEvent::ClientDisconnected(conn.remote_node_id())).await.unwrap();
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_stream(node_id: NodeId, mut stream: Stream, event_sender: mpsc::Sender<ServerEvent>, state: RelayServerState) -> Result<()> {
        tracing::info!("Handling stream ID: {}", stream.stream_id());
        loop {
            match stream.receive_frame().await {
                Ok(frame) => {
                    tracing::info!("Received frame: {:?}", frame);
                    match frame.header.frame_type {
                        FrameType::RegisterRequest => {
                            let req: RegisterRequest = RegisterRequest::from_bytes(&frame.payload)?;
                            match req {
                                RegisterRequest::ControlStream(node_addr) => {
                                    tracing::info!("Control stream request from node: {:?}", node_addr);
                                    // Register the control stream
                                    let mut nodes = state.nodes.write().await;
                                    nodes.insert(node_id, node_addr);
                                    drop(nodes);
                                    tracing::info!("Registered control stream for node: {:?}", node_id);
                                    // Send a response
                                    let res = RegisterResponse::ControlStreamAccepted;
                                    let response_frame = Frame::builder()
                                        .as_response()
                                        .with_frame_type(FrameType::RegisterResponse)
                                        .with_payload(res.to_bytes()?)
                                        .build();
                                    if let Err(e) = stream.send_frame(response_frame).await {
                                        tracing::info!("Error sending register response: {:?}", e);
                                        break;
                                    }
                                    tracing::info!("Sent register response to {}", node_id);
                                    // Notify the event sender
                                    event_sender.send(ServerEvent::ClientRegistered(node_id)).await.unwrap();

                                    let (frame_sender, frame_receiver) = mpsc::channel(100);
                                    let mut control_streams = state.control_streams.write().await;
                                    control_streams.insert(node_id, frame_sender);
                                    drop(control_streams);
                                    // Handle the control stream
                                    let state = state.clone();
                                    let event_sender = event_sender.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = RelayServerActor::handle_control_stream(node_id, stream, event_sender, frame_receiver, state).await {
                                            tracing::info!("Error handling tunnel stream: {:?}", e);
                                        }
                                    });
                                    break;
                                },
                                RegisterRequest::DataStream(tunnel_id) => {
                                    tracing::info!("Data stream request for tunnel ID: {:?}", tunnel_id);
                                    // Handle data stream registration
                                    let mut pending_streams = state.pending_streams.write().await;
                                    if let Some(res_tx) = pending_streams.remove(&tunnel_id) {
                                        // Send a response to the pending stream
                                        let res = RegisterResponse::DataStreamAccepted(tunnel_id);
                                        let res_frame = Frame::builder()
                                            .as_response()
                                            .with_frame_type(FrameType::RegisterResponse)
                                            .with_payload(res.to_bytes()?)
                                            .build();
                                        if let Err(e) = stream.send_frame(res_frame).await {
                                            tracing::info!("Error sending data stream response: {:?}", e);
                                            break;
                                        }
                                        match res_tx.send(Ok(stream)) {
                                            Ok(_) => tracing::info!("Data stream accepted for tunnel ID: {:?}", tunnel_id),
                                            Err(e) => {
                                                match e {
                                                    Ok(_stream) => {
                                                        // TODO!
                                                    },
                                                    Err(e) => tracing::info!("Error sending data stream response: {:?}", e),
                                                }
                                            },
                                        }
                                        break;
                                    } else {
                                        // No pending stream found, send a rejection
                                        let res = RegisterResponse::Rejected("No pending stream found".to_string());
                                        let response_frame = Frame::builder()
                                            .as_response()
                                            .with_frame_type(FrameType::RegisterResponse)
                                            .with_payload(res.to_bytes()?)
                                            .build();
                                        if let Err(e) = stream.send_frame(response_frame).await {
                                            tracing::info!("Error sending register rejection: {:?}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                        },
                        FrameType::TunnelRequest => {
                            let req: TunnelRequest = TunnelRequest::from_bytes(&frame.payload)?;
                            let req_clone = req.clone();
                            match req {
                                TunnelRequest::Create(tun) => {
                                    tracing::info!("Tunnel request from {} to {}", tun.src_node, tun.dst_node);
                                    // Register the pending stream
                                    let (res_tx, res_rx) = oneshot::channel();
                                    let mut pending_streams = state.pending_streams.write().await;
                                    pending_streams.insert(tun.tunnel_id, res_tx);
                                    drop(pending_streams);
                                    // Check if the control stream for the destination node exists
                                    let control_streams = state.control_streams.read().await;
                                    if let Some(frame_sender) = control_streams.get(&tun.dst_node) {
                                        let relay_frame = Frame::builder()
                                            .as_request()
                                            .with_frame_type(FrameType::TunnelRequest)
                                            .with_payload(req_clone.to_bytes()?)
                                            .build();
                                        if let Err(e) = frame_sender.send(relay_frame).await {
                                            tracing::info!("Error relaying tunnel request to control stream: {:?}", e);
                                            break;
                                        }
                                        tracing::info!("Relayed tunnel request to control stream of node {}", tun.dst_node);
                                    } else {
                                        tracing::info!("No control stream found for node {}", tun.dst_node);
                                    }
                                    match res_rx.await {
                                        Ok(Ok(data_stream)) => {
                                            // Establish the tunnel
                                            let mut tunnels = state.tunnels.write().await;
                                            tunnels.insert(tun.tunnel_id, TunnelStat::new(tun.src_node, tun.dst_node));
                                            drop(tunnels);
                                            event_sender.send(ServerEvent::TunnelEstablished { src_node: tun.src_node, dst_node: tun.dst_node }).await.unwrap();
                                            tracing::info!("Tunnel established from {} to {}", tun.src_node, tun.dst_node);
                                            // Send a response to the client
                                            let res = TunnelResponse::Created(tun.tunnel_id);
                                            let response_frame = Frame::builder()
                                                .as_response()
                                                .with_frame_type(FrameType::TunnelResponse)
                                                .with_payload(res.to_bytes()?)
                                                .build();
                                            stream.send_frame(response_frame).await?;
                                            // Handle the tunnel stream
                                            let state = state.clone();
                                            let event_sender = event_sender.clone();
                                            tokio::spawn(async move {
                                                if let Err(e) = RelayServerActor::handle_tunnel_stream(tun.tunnel_id, stream, data_stream, event_sender, state).await {
                                                    tracing::info!("Error handling tunnel stream: {:?}", e);
                                                }
                                            });
                                            tracing::info!("Tunnel stream handler spawned for tunnel ID: {}", tun.tunnel_id);
                                            break;
                                        }
                                        Ok(Err(e)) => {
                                            event_sender.send(ServerEvent::TunnelFailed { src_node: tun.src_node, dst_node: tun.dst_node, reason: e.to_string() }).await.unwrap();
                                            tracing::info!("Failed to establish tunnel from {} to {}: {:?}", tun.src_node, tun.dst_node, e);
                                        }
                                        Err(_) => {
                                            event_sender.send(ServerEvent::TunnelFailed { src_node: tun.src_node, dst_node: tun.dst_node, reason: "Timeout".to_string() }).await.unwrap();
                                            tracing::info!("Timed out waiting for tunnel response from {} to {}", tun.src_node, tun.dst_node);
                                        }
                                    }
                                },
                                _ => {
                                    tracing::info!("Received unknown tunnel request: {:?}", req);
                                    continue;
                                }
                            }
                            
                        },
                        _ => {
                            tracing::info!("Unknown frame type: {:?}", frame.header.frame_type);
                        }
                    }
                }
                Err(e) => {
                    tracing::info!("Error receiving frame: {:?}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_control_stream(node_id: NodeId, mut stream: Stream, event_sender: mpsc::Sender<ServerEvent>, mut frame_receiver: mpsc::Receiver<Frame>, state: RelayServerState) -> Result<()> {
        tracing::info!("Handling control stream ID: {}", stream.stream_id());
        loop {
            tokio::select! {
                frame = frame_receiver.recv() => {
                    // Handle outgoing control frames
                    match frame {
                        Some(frame) => {
                            tracing::info!("Received control frame: {:?}", frame);
                            if let Err(e) = stream.send_frame(frame).await {
                                tracing::info!("Error sending control frame: {:?}", e);
                                break;
                            }
                        }
                        None => {
                            tracing::info!("Control stream closed");
                            break;
                        }
                    }
                }
                _ = stream.receive_frame() => {
                    // Handle incoming frames
                    match stream.receive_frame().await {
                        Ok(frame) => {
                            tracing::info!("Received control frame: {:?}", frame);
                            // TODO!: Process the control frame
                        }
                        Err(e) => {
                            tracing::info!("Error receiving control frame: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }
        // Clean up the control stream
        let mut control_streams = state.control_streams.write().await;
        control_streams.remove(&node_id);
        // Notify the event sender that the control stream is closed
        event_sender.send(ServerEvent::ClientUnregistered(node_id)).await.unwrap();
        // Log the closure of the control stream
        tracing::info!("Control stream ID: {} closed", stream.stream_id());
        Ok(())
    }

    async fn handle_tunnel_stream(
        tunnel_id: TunnelId,
        src_stream: Stream,
        dst_stream: Stream,
        event_sender: mpsc::Sender<ServerEvent>,
        state: RelayServerState,
    ) -> Result<()> {
        let (mut src_writer, mut src_reader) = src_stream.split();
        let (mut dst_writer, mut dst_reader) = dst_stream.split();

        let t1 = tokio::spawn(async move {
            let res = tokio::io::copy(&mut src_reader, &mut dst_writer).await;
            tracing::info!("Copy src -> dst: {:?}", res);
            let _ = dst_writer.shutdown().await;
        });

        let t2 = tokio::spawn(async move {
            let res = tokio::io::copy(&mut dst_reader, &mut src_writer).await;
            tracing::info!("Copy dst -> src: {:?}", res);
            let _ = src_writer.shutdown().await;
        });

        let tunnels = state.tunnels.read().await;
        if let Some(tunnel_info) = tunnels.get(&tunnel_id) {
            match event_sender.send(ServerEvent::TunnelEstablished {
                src_node: tunnel_info.src_node,
                dst_node: tunnel_info.dst_node,
            }).await {
                Ok(_) => tracing::info!("Tunnel established event sent for tunnel ID: {}", tunnel_id),
                Err(e) => tracing::info!("Failed to send tunnel established event: {:?}", e),
            }
            tracing::info!("Handling tunnel stream from {} to {}", tunnel_info.src_node, tunnel_info.dst_node);
        } else {
            tracing::info!("Tunnel ID {} not found in state", tunnel_id);
            return Err(anyhow::anyhow!("Tunnel ID not found"));
        }
        // Wait for both directions to finish
        let _ = tokio::try_join!(t1, t2);
        // Clean up tunnel
        {
            let mut tunnels = state.tunnels.write().await;
            tunnels.remove(&tunnel_id);
        }
        tracing::info!("Tunnel {} closed", tunnel_id);
        Ok(())
    }
}

pub struct RelayServer {
    config: TransportConfig,
    relay_addrs: HashSet<StackAddr>,
    event_receiver: mpsc::Receiver<ServerEvent>,
    cmd_sender: mpsc::Sender<ServerCommand>,
    cancel: CancellationToken,
}

impl RelayServer {
    pub fn builder() -> RelayServerBuilder {
        RelayServerBuilder::default()
    }

    /// Get the node ID of the server.
    /// This is the public key of the server's keypair.
    pub fn node_id(&self) -> NodeId {
        self.config.keypair().public().into()
    }

    /// Return current (relay) node address for the server.
    pub fn node_addr(&self) -> RelayAddr {
        RelayAddr {
            node_id: self.node_id(),
            addresses: self.relay_addrs.iter().cloned().collect(),
        }
    }

    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    pub fn addrs(&self) -> &HashSet<StackAddr> {
        &self.relay_addrs
    }

    pub async fn send_command(&self, cmd: ServerCommand) -> Result<()> {
        self.cmd_sender.send(cmd).await?;
        Ok(())
    }

    pub async fn next_event(&mut self) -> Option<ServerEvent> {
        self.event_receiver.recv().await
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.cmd_sender.send(ServerCommand::Shutdown).await?;
        self.cancel.cancel();
        Ok(())
    }
}

pub struct RelayServerBuilder {
    config: TransportConfig,
    addrs: HashSet<StackAddr>,
    protocols: Vec<TransportKind>,
    allow_loopback: bool,
}

impl Default for RelayServerBuilder {
    fn default() -> Self {
        let keypair = Keypair::generate();
        let config = TransportConfig::new(keypair.clone()).unwrap();
        
        let mut protocols = Vec::new();
        protocols.push(TransportKind::Quic);

        Self {
            config,
            addrs: HashSet::new(),
            protocols: protocols,
            allow_loopback: false,
        }
    }
}

impl RelayServerBuilder {
    pub fn with_config(mut self, config: TransportConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_addr(mut self, addr: StackAddr) -> Self {
        self.addrs.insert(addr);
        self
    }

    pub fn with_addrs(mut self, addr: &[StackAddr]) -> Self {
        for a in addr {
            self.addrs.insert(a.clone());
        }
        self
    }

    fn push_protocol(&mut self, proto: TransportKind) {
        if !self.protocols.contains(&proto) {
            self.protocols.push(proto);
        }
    }

    pub fn with_protocol(mut self, protocol: TransportKind) -> Self {
        self.push_protocol(protocol);
        self
    }

    pub fn with_allow_loopback(mut self, allow_loopback: bool) -> Self {
        self.allow_loopback = allow_loopback;
        self
    }

    pub fn build(self) -> Result<RelayServer> {
        let (event_sender, event_receiver) = mpsc::channel(100);
        let (cmd_sender, cmd_receiver) = mpsc::channel(100);
        let cancel = CancellationToken::new();

        let state = RelayServerState::new();

        let addrs = if self.addrs.is_empty() {
            get_unspecified_stack_addrs(&self.protocols)
        } else {
            self.addrs.clone()
        };

        let mut actor = RelayServerActor {
            config: self.config.clone(),
            addrs: addrs,
            event_sender: event_sender,
            cmd_receiver,
            state,
            cancel: cancel.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = actor.run().await {
                tracing::error!("Relay server actor failed: {:?}", e);
            }
        });

        let direct_addrs = if self.addrs.is_empty() {
            get_default_stack_addrs(&self.protocols, self.allow_loopback)
        } else {
            replace_with_actual_addrs(&self.addrs, &self.protocols, self.allow_loopback)
        };

        Ok(RelayServer {
            config: self.config,
            relay_addrs: direct_addrs,
            event_receiver,
            cmd_sender,
            cancel,
        })
    }
}
