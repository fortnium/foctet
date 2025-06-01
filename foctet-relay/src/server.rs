use foctet_core::{addr::node::NodeAddr, frame::{Frame, FrameType}, id::NodeId, proto::relay::{RegisterRequest, RegisterResponse, TunnelId, TunnelRequest, TunnelResponse}, transport::ListenerId};
use foctet_net::{config::TransportConfig, transport::{connection::{Connection, ConnectionEvent}, quic::transport::QuicTransport, stream::Stream, tcp::transport::TcpTransport, Transport}};
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
    Start,
    Stop,
}

#[derive(Clone)]
pub struct RelayServerState {
    pub nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    pub pending_streams: Arc<RwLock<HashMap<TunnelId, oneshot::Sender<Result<Stream>>>>>,
    pub control_streams: Arc<RwLock<HashMap<NodeId, mpsc::Sender<Frame>>>>,
    pub tunnels: Arc<RwLock<HashMap<TunnelId, TunnelStat>>>,
}

pub struct RelayServer {
    config: TransportConfig,
    addrs: HashSet<StackAddr>,
    cmd_receiver: mpsc::Receiver<ServerCommand>,
    state: RelayServerState,
    cancel: CancellationToken,
}

impl RelayServer {
    pub async fn run(&mut self) -> Result<()> {
        let mut listerner_id = ListenerId::new(1);
        // Create a channel for endpoint events
        let (event_sender, mut event_receiver): (mpsc::Sender<ServerEvent>, mpsc::Receiver<ServerEvent>) = mpsc::channel(100);
        for addr in &self.addrs {
            let config = self.config.clone();
            let mut transport: Transport = match addr.transport() {
                Some(transport) => match transport {
                    TransportProtocol::Quic(_) | TransportProtocol::Udp(_) => {
                        let t = QuicTransport::new(config)?;
                        Transport::Quic(t)
                    },
                    TransportProtocol::TlsOverTcp(_) | TransportProtocol::Tcp(_) => {
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
            let event_sender = event_sender.clone();
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
                                let _ = RelayServer::handle_connection(&mut conn, event_sender, state).await;
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
                        ServerCommand::Start => {
                            tracing::info!("RelayServer started");
                        }
                        ServerCommand::Stop => {
                            tracing::info!("RelayServer stopped");
                            break;
                        }
                    }
                }
                Some(event) = event_receiver.recv() => {
                    match event {
                        ServerEvent::ClientConnected(client_info) => {
                            tracing::info!("Client connected: {:?}", client_info);
                        }
                        ServerEvent::ClientDisconnected(node_id) => {
                            tracing::info!("Client disconnected: {:?}", node_id);
                        }
                        ServerEvent::ClientRegistered(client_info) => {
                            tracing::info!("Client registered: {:?}", client_info);
                        }
                        ServerEvent::ClientUnregistered(node_id) => {
                            tracing::info!("Client unregistered: {:?}", node_id);
                        }
                        ServerEvent::TunnelEstablished { src_node, dst_node } => {
                            tracing::info!("Tunnel established from {} to {}", src_node, dst_node);
                        }
                        ServerEvent::TunnelFailed { src_node, dst_node, reason } => {
                            tracing::error!("Tunnel failed from {} to {}: {}", src_node, dst_node, reason);
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
                        let _ = RelayServer::handle_stream(node_id, stream, event_sender, state).await;
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
                                    tracing::info!("Handling control stream ID: {}", stream.stream_id());

                                    let (frame_sender, frame_receiver) = mpsc::channel(100);
                                    let mut control_streams = state.control_streams.write().await;
                                    control_streams.insert(node_id, frame_sender);
                                    drop(control_streams);
                                    // Handle the control stream
                                    let state = state.clone();
                                    let event_sender = event_sender.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = RelayServer::handle_control_stream(node_id, stream, event_sender, frame_receiver, state).await {
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
                                                if let Err(e) = RelayServer::handle_tunnel_stream(tun.tunnel_id, stream, data_stream, event_sender, state).await {
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

pub struct Server {
    // config: TransportConfig,
    // relay_addrs: HashSet<StackAddr>,
    // nodes: Arc<RwLock<HashMap<NodeId, NodeAddr>>>,
    // relay_event_receiver: mpsc::Receiver<ServerEvent>,
    // relay_cmd_sender: mpsc::Sender<ServerCommand>,
    // relay_server_task: AbortOnDropHandle<()>,
    // stun_server_task: AbortOnDropHandle<()>,
    // cancel: CancellationToken,
}

impl Server {
    
}
