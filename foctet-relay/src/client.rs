use foctet_core::{addr::node::{NodeAddr, RelayAddr}, frame::{Frame, FrameType}, id::NodeId, key::Keypair, proto::relay::{RegisterRequest, RegisterResponse, TunnelId, TunnelInfo, TunnelRequest, TunnelResponse}, transport::TransportKind};
use foctet_net::{config::TransportConfig, transport::{quic::transport::QuicTransport, stream::Stream, tcp::transport::TcpTransport, Transport}};
use stackaddr::{segment::protocol::TransportProtocol};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use anyhow::{anyhow, Result};

pub enum RelayCommand {
    OpenStream(NodeId, oneshot::Sender<Result<Stream>>),
    SendControlMessage(Frame),
    Shutdown,
}

pub struct RelayClientActor {
    local_node_addr: NodeAddr,
    relay_addr: RelayAddr,
    transport: Transport,
    stream_sender: mpsc::Sender<Stream>,
    cmd_receiver: mpsc::Receiver<RelayCommand>,
    cancel: CancellationToken,
    allow_loopback: bool,
}

impl RelayClientActor {
    pub async fn run(mut self) -> Result<()> {
        // TODO: Select stack address from relay address which matches the transport protocol
        // and connect to the relay server.
        let addrs = self.relay_addr.get_direct_addrs(&self.transport.transport_kind(), self.allow_loopback);
        if addrs.is_empty() {
            return Err(anyhow!("No valid addresses found for relay connection"));
        }
        // Attempt to connect to the all addresses in the relay address.
        let mut connection = {
            let mut conn_opt = None;
            for addr in addrs {
                match self.transport.connect(addr).await {
                    Ok(conn) => {
                        conn_opt = Some(conn);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to connect to relay: {:?}", e);
                    }
                }
            }
            match conn_opt {
                Some(conn) => conn,
                None => return Err(anyhow!("Failed to connect to any relay address")),
            }
        };
        tracing::info!("Connected to relay: {}", connection.remote_addr());

        // Create a new stream for the relay control channel.
        let mut stream = connection.open_stream().await?;
        tracing::info!("Opened control stream to relay: {}", connection.remote_addr());
        // Send the local node address to the relay server.
        let reg_req = RegisterRequest::ControlStream(self.local_node_addr.clone()).to_bytes()?;
        let regist_frame = Frame::builder()
            .with_frame_type(FrameType::RegisterRequest)
            .with_payload(reg_req)
            .build();
        stream.send_frame(regist_frame).await?;
        tracing::info!("Sent register request to relay: {}", connection.remote_addr());
        // Wait for the relay server to respond with a confirmation.
        let response_frame = stream.receive_frame().await?;
        if response_frame.header.frame_type != FrameType::RegisterResponse {
            return Err(anyhow!("Expected RegisterResponse frame, got {:?}", response_frame.header.frame_type));
        }
        tracing::info!("Received register response from relay: {}", connection.remote_addr());
        // Process the response frame to extract the relay node ID.

        let reg_res = RegisterResponse::from_bytes(&response_frame.payload)?;

        match reg_res {
            RegisterResponse::ControlStreamAccepted => {
                tracing::info!("Control stream accepted by relay: {}", connection.remote_addr());
            },
            RegisterResponse::Rejected(reason) => {
                return Err(anyhow!("Control stream rejected by relay: {}", reason));
            },
            _ => {
                return Err(anyhow!("Unexpected register response frame: {:?}", reg_res));
            },
        }

        let (mut send_stream, mut recv_stream) = stream.split();
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    tracing::info!("RelayClientActor loop cancelled, closing loop");
                    break;
                }
                Some(cmd) = self.cmd_receiver.recv() => {
                    match cmd {
                        RelayCommand::OpenStream(node_id, stream_responder) => {
                            // Open a new stream to the relay server for the specified node ID.
                            let mut new_stream = connection.open_stream().await?;
                            let tunnel_id = TunnelId::new();
                            let tunnel_info = TunnelInfo::new(self.local_node_addr.node_id, node_id, tunnel_id);
                            let req = TunnelRequest::Create(tunnel_info);
                            let open_frame = Frame::builder()
                                .as_request()
                                .with_frame_type(FrameType::TunnelRequest)
                                .with_payload(req.to_bytes()?)
                                .build();
                            new_stream.send_frame(open_frame).await?;

                            // Spawn a new task to wait for the tunnel response.
                            // If the tunnel is created, send the stream back to the caller via the responder.
                            tokio::spawn(async move {
                                match new_stream.receive_frame().await {
                                    Ok(frame) => {
                                        match frame.header.frame_type  {
                                            FrameType::TunnelResponse => {
                                                let res = TunnelResponse::from_bytes(&frame.payload);
                                                match res {
                                                    Ok(TunnelResponse::Created(tunnel_id)) => {
                                                        tracing::info!("Tunnel created: {}", tunnel_id);
                                                        // Send the new stream back to the caller.
                                                        if let Err(_e) = stream_responder.send(Ok(new_stream)) {
                                                            tracing::error!("Failed to send stream response");
                                                        }
                                                    },
                                                    Ok(TunnelResponse::Closed(_)) => {
                                                        tracing::warn!("Tunnel closed before creation");
                                                        if let Err(_e) = stream_responder.send(Err(anyhow!("Tunnel closed before creation"))) {
                                                            tracing::error!("Failed to send stream response");
                                                        }
                                                    },
                                                    _ => {
                                                        tracing::warn!("Unexpected TunnelResponse type: {:?}", res);
                                                        if let Err(_e) = stream_responder.send(Err(anyhow!("Unexpected TunnelResponse type"))) {
                                                            tracing::error!("Failed to send stream response");
                                                        }
                                                    },
                                                }
                                            }
                                            _ => {
                                                tracing::warn!("Received unexpected frame type: {:?}", frame.header.frame_type);
                                                if let Err(_e) = stream_responder.send(Err(anyhow!("Unexpected frame type received"))) {
                                                    tracing::error!("Failed to send stream response");
                                                }
                                            },
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("Failed to receive tunnel response frame: {:?}", e);
                                        if let Err(_e) = stream_responder.send(Err(e)) {
                                            tracing::error!("Failed to send stream response");
                                        }
                                    },
                                }
                            });
                        },
                        RelayCommand::SendControlMessage(frame) => {
                            // Send a control message frame to the relay server.
                            send_stream.send_frame(frame).await?;
                        },
                        RelayCommand::Shutdown => {
                            tracing::info!("RelayClientActor received shutdown command, closing connection");
                            break;
                        },
                    }
                }
                Ok(frame) = recv_stream.receive_frame() => {
                    // Process incoming frames from the relay server.
                    match frame.header.frame_type {
                        FrameType::TunnelRequest => {
                            // Handle tunnel request frames from other nodes via the relay server.
                            tracing::info!("Received TunnelRequest frame from relay: {}", connection.remote_addr());
                            let req = TunnelRequest::from_bytes(&frame.payload)?;
                            match req {
                                TunnelRequest::Create(tunnel_info) => {
                                    // TODO: Validate the tunnel request.
                                    // Create a new stream for the tunnel request.
                                    let mut new_stream = connection.open_stream().await?;
                                    let reg_req = RegisterRequest::DataStream(tunnel_info.tunnel_id).to_bytes()?;
                                    let reg_frame = Frame::builder()
                                        .with_frame_type(FrameType::RegisterRequest)
                                        .with_payload(reg_req)
                                        .build();
                                    new_stream.send_frame(reg_frame).await?;
                                    self.stream_sender.send(new_stream).await?;
                                    tracing::info!("Created new stream for tunnel request: {}", tunnel_info);
                                },
                                _ => {
                                    tracing::warn!("Received unexpected TunnelRequest type: {:?}", req);
                                },
                            }
                        },
                        _ => {
                            // Handle other frame types as needed.
                            tracing::warn!("Received unexpected frame type: {:?}", frame.header.frame_type);
                        },
                    }
                }
                else => {
                    // Handle stream closure or errors.
                    tracing::warn!("Stream closed or error occurred, exiting loop");
                    break;
                }
            }
        }
        Ok(())
    }
}

pub struct RelayClient {
    pub local_node_addr: NodeAddr,
    pub relay_addr: RelayAddr,
    pub stream_receiver: mpsc::Receiver<Stream>,
    pub cmd_sender: mpsc::Sender<RelayCommand>,
    cancel: CancellationToken,
}

impl RelayClient {
    pub fn builder() -> RelayClientBuilder {
        RelayClientBuilder::default()
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.cmd_sender.send(RelayCommand::Shutdown).await?;
        self.cancel.cancel();
        Ok(())
    }
}

pub struct RelayClientBuilder {
    config: TransportConfig,
    protocol: Option<TransportKind>,
    local_node_addr: NodeAddr,
    relay_addr: RelayAddr,
    allow_loopback: bool,
}

impl Default for RelayClientBuilder {
    fn default() -> Self {
        let keypair = Keypair::generate();
        let config = TransportConfig::new(keypair.clone()).unwrap();

        Self {
            config,
            protocol: None,
            local_node_addr: NodeAddr::default(),
            relay_addr: RelayAddr::default(),
            allow_loopback: false,
        }
    }
}

impl RelayClientBuilder {

    pub fn with_config(mut self, config: TransportConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_protocol(mut self, protocol: TransportKind) -> Self {
        self.protocol = Some(protocol);
        self
    }

    pub fn with_local_node_addr(mut self, local_node_addr: NodeAddr) -> Self {
        self.local_node_addr = local_node_addr;
        self
    }

    pub fn with_relay_addr(mut self, relay_addr: RelayAddr) -> Self {
        self.relay_addr = relay_addr;
        self
    }

    pub fn with_allow_loopback(mut self, allow_loopback: bool) -> Self {
        self.allow_loopback = allow_loopback;
        self
    }

    /// Builds the RelayClient with the provided configuration, 
    /// spawns the RelayClientActor and returns the RelayClient.
    pub fn build(self) -> Result<RelayClient> {
        if self.relay_addr.addresses.is_empty() {
            return Err(anyhow!("Relay address must contain at least one address"));
        }
        if self.local_node_addr.node_id.is_zero() {
            return Err(anyhow!("Local node address must have a valid node ID"));
        }
        if self.relay_addr.node_id.is_zero() {
            return Err(anyhow!("Relay address must have a valid node ID"));
        }

        // Select the transport protocol based on the relay address
        let mut protocol = TransportKind::Quic;
        if let Some(proto) = self.protocol {
            protocol = proto;
        } else {
            for addr in &self.relay_addr.addresses {
                if let Some(proto) = addr.transport() {
                    match proto {
                        TransportProtocol::Quic(_) | TransportProtocol::Udp(_) => {
                            protocol = TransportKind::Quic;
                            break;
                        },
                        TransportProtocol::TlsTcp(_) | TransportProtocol::Tcp(_) => {
                            protocol = TransportKind::TlsTcp;
                            break;
                        },
                        _ => {}
                    }
                }
            }
        }
        
        let transport = match protocol {
            TransportKind::Quic => {
                let t = QuicTransport::new(self.config.clone())?;
                Transport::Quic(t)
            },
            TransportKind::TlsTcp => {
                let t = TcpTransport::new(self.config.clone())?;
                Transport::Tcp(t)
            },
        };

        // Create a channel for streams
        let (stream_sender, stream_receiver) = mpsc::channel(100);
        // Create a channel for commands
        let (cmd_sender, cmd_receiver) = mpsc::channel(100);
        let cancel = CancellationToken::new();
        
        let actor = RelayClientActor {
            local_node_addr: self.local_node_addr.clone(),
            relay_addr: self.relay_addr.clone(),
            transport,
            stream_sender,
            cmd_receiver,
            cancel: cancel.clone(),
            allow_loopback: self.allow_loopback,
        };

        // Spawn the RelayClientActor
        tokio::spawn(async move {
            if let Err(e) = actor.run().await {
                tracing::error!("RelayClientActor error: {:?}", e);
            }
        });

        Ok(RelayClient {
            local_node_addr: self.local_node_addr,
            relay_addr: self.relay_addr,
            stream_receiver,
            cmd_sender,
            cancel,
        })
    }
}
