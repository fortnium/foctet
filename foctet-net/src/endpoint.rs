use crate::{
    config::TransportConfig, device, transport::{
        connection::{Connection, ConnectionEvent}, quic::transport::QuicTransport, tcp::transport::TcpTransport, Transport,
    }
};
use anyhow::{anyhow, Result};
use bytes::Bytes;
use foctet_core::{addr::node::{NodeAddr, RelayAddr}, default, id::NodeId, ip, key::Keypair, transport::{ListenerId, TransportKind}};
use stackaddr::{segment::protocol::TransportProtocol, Identity, Protocol, StackAddr};
use tokio_util::sync::CancellationToken;
use std::{
    collections::{BTreeMap, HashMap, HashSet}, net::{IpAddr, Ipv4Addr}, sync::Arc
};
use tokio::sync::{mpsc, Mutex};

pub struct ListenerHandle {
    conn_receiver: Arc<Mutex<mpsc::Receiver<Connection>>>,
}

impl ListenerHandle {
    pub fn new(conn_receiver: Arc<Mutex<mpsc::Receiver<Connection>>>) -> Self {
        Self { conn_receiver }
    }

    pub async fn accept(&self) -> Option<Connection> {
        self.conn_receiver.lock().await.recv().await
    }

    pub async fn clone(&self) -> Self {
        Self {
            conn_receiver: Arc::clone(&self.conn_receiver),
        }
    }
}

pub struct RelayActor {

}

pub struct EndpointActor {
    config: TransportConfig,
    addrs: HashSet<StackAddr>,
    conn_sender: mpsc::Sender<Connection>,
    event_sender: mpsc::Sender<EndpointEvent>,
    cmd_receiver: mpsc::Receiver<EndpointCommand>,
    cancel: CancellationToken,
    listen_enabled: bool,
}

impl EndpointActor {
    pub async fn run(mut self) -> Result<()> {
        // Create a listener for each address
        if self.listen_enabled {
            let mut listerner_id = ListenerId::new(1);
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
                let event_sender = self.event_sender.clone();
                let conn_sender = self.conn_sender.clone();
                let mut listener = transport.listen_on(listerner_id.fetch_add(1), addr.clone()).await?;
                tokio::spawn(async move {
                    while let Some(conn_event) = listener.accept().await {
                        match conn_event {
                            ConnectionEvent::Accepted(conn) => {
                                match conn_sender.send(conn).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        event_sender
                                            .send(EndpointEvent::Error(anyhow!("Error sending connection event: {:?}", e)))
                                            .await
                                            .unwrap_or_else(|e| {
                                                tracing::error!("Error sending connection event: {:?}", e);
                                            });
                                    }
                                }
                            }
                            _ => {},
                        }
                    }
                });
            }
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
                        EndpointCommand::Connect(_addr) => {
                            // Handle connect command
                            // TODO!: Additional logic to handle connection
                            // For now, connect via Endpoint::connect
                        }
                        EndpointCommand::Listen(_addr) => {
                            // Handle listen command
                            // TODO!: Additional logic to handle listening on a new address
                        }
                        EndpointCommand::Shutdown => {
                            // Handle shutdown command
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// The endpoint for network communication.
/// This is the main entry point for establishing connections and listening for incoming connections.
pub struct Endpoint {
    config: TransportConfig,
    addrs: HashSet<StackAddr>,
    relay_addrs: Option<RelayAddr>,
    priority_map: BTreeMap<u8, TransportKind>,
    transports: HashMap<TransportKind, Transport>,
    listener: ListenerHandle,
    event_receiver: mpsc::Receiver<EndpointEvent>,
    cmd_sender: mpsc::Sender<EndpointCommand>,
    cancel: CancellationToken,
    allow_loopback: bool,
}

impl Endpoint {
    /// Create a new endpoint builder for building an endpoint.
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder::new()
    }

    /// Create a new endpoint with the default configuration.
    pub fn default_builder() -> EndpointBuilder {
        EndpointBuilder::default()
    }

    /// Get the node ID of the endpoint.
    /// This is the public key of the endpoint's keypair.
    pub fn node_id(&self) -> NodeId {
        self.config.keypair().public().into()
    }

    /// Return current node address for the endpoint.
    pub fn node_addr(&self) -> NodeAddr {
        NodeAddr {
            node_id: self.node_id(),
            addresses: self.addrs.iter().cloned().collect(),
            relay_addr: self.relay_addrs.clone(),
        }
    }

    /// Return global-only node address for the endpoint.
    pub fn global_node_addr(&self) -> NodeAddr {
        let global_addrs: Vec<StackAddr> = self
            .addrs
            .iter()
            .cloned()
            .filter(|addr| {
                if let Some(ip) = addr.ip() {
                    ip::is_global_ip(&ip)
                } else {
                    false
                }
            })
            .collect();

        NodeAddr {
            node_id: self.node_id(),
            addresses: global_addrs.into_iter().collect(),
            relay_addr: self.relay_addrs.clone(),
        }
    }

    /// Connect to a remote node using the given StackAddr.
    pub async fn connect(&mut self, addr: StackAddr) -> Result<Connection> {
        match addr.transport() {
            Some(transport) => {
                match transport {
                    TransportProtocol::Quic(_) | TransportProtocol::Udp(_) => {
                        let t = self.transports.get_mut(&TransportKind::Quic).ok_or_else(|| anyhow!("QUIC transport not found"))?;
                        t.connect(addr).await
                    },
                    TransportProtocol::TlsOverTcp(_) | TransportProtocol::Tcp(_) => {
                        let t = self.transports.get_mut(&TransportKind::TlsOverTcp).ok_or_else(|| anyhow!("TCP transport not found"))?;
                        t.connect(addr).await
                    },
                    _ => Err(anyhow!("Unsupported transport protocol: {:?}", transport)),
                }
            }
            None => Err(anyhow!("Missing transport protocol in address")),
        }
    }

    /// Connect to a remote node using the given NodeAddr.
    pub async fn connect_node(&mut self, addr: NodeAddr) -> Result<Connection> {
        let iface = &netdev::get_default_interface()
            .map_err(|e| anyhow!("Failed to get default interface: {:?}", e))?;
        for proto in self.priority_map.values() {
            let t = self.transports.get_mut(proto).ok_or_else(|| anyhow!("Transport not found"))?;
            let addrs = addr.get_direct_addrs(proto, self.allow_loopback);
            let sorted_addrs = device::sort_addrs_by_reachability(&addrs, iface);
            for addr in sorted_addrs {
                match t.connect(addr.clone()).await {
                    Ok(conn) => {
                        return Ok(conn);
                    }
                    Err(e) => {
                        tracing::error!("Error connecting to {}: {:?}", addr, e);
                    }
                }
            }
        }
        Err(anyhow!("No direct address found for node"))
    }

    pub async fn accept(&mut self) -> Option<Connection> {
        self.listener.accept().await
    }

    pub async fn get_listener(&self) -> ListenerHandle {
        self.listener.clone().await
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.cmd_sender.send(EndpointCommand::Shutdown).await?;
        self.cancel.cancel();
        Ok(())
    }

    pub async fn send_command(&self, cmd: EndpointCommand) -> Result<()> {
        self.cmd_sender.send(cmd).await?;
        Ok(())
    }

    pub async fn next_event(&mut self) -> Option<EndpointEvent> {
        self.event_receiver.recv().await
    }
}

/// Represents an event that occurs in the endpoint.
#[derive(Debug)]
pub enum EndpointEvent {
    ConnectionEstablished {
        node_id: NodeId,
        addr: StackAddr,
    },
    ConnectionClosed {
        node_id: NodeId,
    },
    NewListenAddr {
        listener_id: ListenerId,
        addr: StackAddr,
    },
    PeerDiscovered {
        node_id: NodeId,
        addr: StackAddr,
    },
    Error(anyhow::Error),
}

pub enum EndpointCommand {
    Connect(StackAddr),
    Listen(StackAddr),
    Shutdown,
}

/// The builder for building an endpoint.
/// Provides methods for configuring the endpoint with builder pattern.
pub struct EndpointBuilder {
    config: TransportConfig,
    protocols: Vec<TransportKind>,
    addrs: HashSet<StackAddr>,
    listen_enabled: bool,
    allow_loopback: bool,
}

impl Default for EndpointBuilder {
    fn default() -> Self {
        let keypair = Keypair::generate();
        let config = TransportConfig::new(keypair.clone()).unwrap();

        let mut protocols = Vec::new();
        protocols.push(TransportKind::Quic);

        // Default stack address
        let mut addrs = HashSet::new();
        let addr = StackAddr::empty()
        .with_protocol(Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
        .with_protocol(Protocol::Udp(default::DEFAULT_SERVER_PORT))
        .with_protocol(Protocol::Quic)
        .with_identity(Identity::NodeId(Bytes::copy_from_slice(&keypair.public().to_bytes())));

        addrs.insert(addr);

        Self { 
            config, 
            protocols, 
            addrs: addrs,
            listen_enabled: true,
            allow_loopback: false,
        }
    }
}

impl EndpointBuilder {
    /// Create a new endpoint builder with the given keypair.
    pub fn new() -> Self {
        let keypair = Keypair::generate();
        let config = TransportConfig::new(keypair.clone()).unwrap();
        Self { 
            config, 
            protocols: Vec::new(), 
            addrs: HashSet::new(), 
            listen_enabled: true, 
            allow_loopback: false 
        }
    }
    pub fn with_keypair(mut self, keypair: Keypair) -> Self {
        self.config.set_keypair(keypair).unwrap();
        self
    }

    fn push_protocol(&mut self, proto: TransportKind) {
        if !self.protocols.contains(&proto) {
            self.protocols.push(proto);
        }
    }

    /// Add QUIC support to the endpoint.
    /// If `addr` is not set, the default address will be used for listening.
    pub fn with_quic(mut self) -> Self {
        self.push_protocol(TransportKind::Quic);
        self
    }
    /// Add TCP support to the endpoint.
    /// If `addr` is not set, the default address will be used for listening.
    pub fn with_tcp(mut self) -> Self {
        self.push_protocol(TransportKind::TlsOverTcp);
        self
    }

    /// Add listen address to the endpoint.
    /// This address will be used for listening for incoming connections.
    pub fn with_addr(mut self, addr: StackAddr) -> Result<Self> {
        let transport = addr.transport().ok_or_else(|| anyhow!("Missing transport protocol in address"))?;
        self.push_protocol(TransportKind::from_protocol(transport)?);
        self.addrs.insert(addr);
        Ok(self)
    }

    /// Set listener disabled.
    /// This will disable the listener for incoming connections.
    pub fn without_listen(mut self) -> Self {
        self.listen_enabled = false;
        self
    }

    /// Set allow loopback.
    pub fn allow_loopback(mut self, allow: bool) -> Self {
        self.allow_loopback = allow;
        self
    }

    /// Set the read buffer size for the endpoint.
    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.config.read_buffer_size = size;
        self
    }

    /// Set the write buffer size for the endpoint.
    pub fn with_write_buffer_size(mut self, size: usize) -> Self {
        self.config.write_buffer_size = size;
        self
    }

    /// Set the maximum read buffer size for the endpoint.
    pub fn with_max_read_buffer_size(mut self) -> Self {
        self.config.read_buffer_size = default::MAX_READ_BUFFER_SIZE;
        self
    }

    /// Set the maximum write buffer size for the endpoint.
    pub fn with_max_write_buffer_size(mut self) -> Self {
        self.config.write_buffer_size = default::MAX_WRITE_BUFFER_SIZE;
        self
    }

    /// Build and spawn the endpoint.
    /// This will create the endpoint and start listening for incoming connections.
    pub fn build(self) -> Result<Endpoint> {
        let mut priority_map = BTreeMap::new();
        let mut transports = HashMap::new();
        for (i, proto) in self.protocols.iter().enumerate() {
            let priority = (i + 1) as u8;
            match proto {
                TransportKind::Quic => {
                    let t = QuicTransport::new(self.config.clone())?;
                    transports.insert(TransportKind::Quic, Transport::Quic(t));
                    priority_map.insert(priority, TransportKind::Quic);
                },
                TransportKind::TlsOverTcp => {
                    let t = TcpTransport::new(self.config.clone())?;
                    transports.insert(TransportKind::TlsOverTcp, Transport::Tcp(t));
                    priority_map.insert(priority, TransportKind::TlsOverTcp);
                },
            }
        }

        let addrs = if self.addrs.is_empty() {
            get_unspecified_stack_addrs(&self.protocols)
        } else {
            self.addrs.clone()
        };

        // Create a channel for connection events
        let (conn_sender, conn_receiver) = mpsc::channel(100);
        // Create a channel for endpoint events
        let (event_sender, event_receiver) = mpsc::channel(100);
        // Create a channel for endpoint commands
        let (cmd_sender, cmd_receiver) = mpsc::channel(100);
        // Create a cancellation token for the endpoint
        let cancel = CancellationToken::new();
        // Create the endpoint actor
        let actor = EndpointActor {
            config: self.config.clone(),
            addrs: addrs,
            conn_sender,
            event_sender,
            cmd_receiver,
            cancel: cancel.clone(),
            listen_enabled: self.listen_enabled,
        };
        // Spawn the endpoint actor
        tokio::spawn(async move {
            if let Err(e) = actor.run().await {
                tracing::error!("Endpoint actor error: {:?}", e);
            }
        });

        let direct_addrs = if self.addrs.is_empty() {
            get_default_stack_addrs(&self.protocols, self.allow_loopback)
        } else {
            replace_with_actual_addrs(&self.addrs, &self.protocols, self.allow_loopback)
        };

        Ok(Endpoint {
            config: self.config,
            addrs: direct_addrs,
            relay_addrs: None,
            priority_map,
            transports,
            listener: ListenerHandle::new(Arc::new(Mutex::new(conn_receiver))),
            event_receiver,
            cmd_sender,
            cancel,
            allow_loopback: self.allow_loopback,
        })
    }
}

fn get_unspecified_stack_addrs(protocols: &[TransportKind]) -> HashSet<StackAddr> {
    let unspecified_addr = device::get_unspecified_server_addr();
    let mut addrs = HashSet::new();
    for proto in protocols.iter() {
        match proto {
            TransportKind::Quic => {
                match unspecified_addr.ip() {
                    IpAddr::V4(ipv4) => {
                        addrs.insert(StackAddr::empty()
                            .with_protocol(Protocol::Ip4(ipv4))
                            .with_protocol(Protocol::Udp(unspecified_addr.port()))
                            .with_protocol(Protocol::Quic));
                    }
                    IpAddr::V6(ipv6) => {
                        addrs.insert(StackAddr::empty()
                            .with_protocol(Protocol::Ip6(ipv6))
                            .with_protocol(Protocol::Udp(unspecified_addr.port()))
                            .with_protocol(Protocol::Quic));
                    }
                }
            }
            TransportKind::TlsOverTcp => {
                match unspecified_addr.ip() {
                    IpAddr::V4(ipv4) => {
                        addrs.insert(StackAddr::empty()
                            .with_protocol(Protocol::Ip4(ipv4))
                            .with_protocol(Protocol::Tcp(unspecified_addr.port()))
                            .with_protocol(Protocol::Tls));
                    }
                    IpAddr::V6(ipv6) => {
                        addrs.insert(StackAddr::empty()
                            .with_protocol(Protocol::Ip6(ipv6))
                            .with_protocol(Protocol::Tcp(unspecified_addr.port()))
                            .with_protocol(Protocol::Tls));
                    }
                }
            }
        }
    }
    addrs
}

fn get_default_stack_addrs(protocols: &[TransportKind], allow_loopback: bool) -> HashSet<StackAddr> {
    let socket_addrs = crate::device::get_default_server_addrs(default::DEFAULT_SERVER_PORT, allow_loopback);
    let mut addrs = HashSet::new();
    for proto in protocols.iter() {
        for addr in socket_addrs.iter() {
            match proto {
                TransportKind::Quic => {
                    match addr.ip() {
                        IpAddr::V4(ipv4) => {
                            addrs.insert(StackAddr::empty()
                                .with_protocol(Protocol::Ip4(ipv4))
                                .with_protocol(Protocol::Udp(addr.port()))
                                .with_protocol(Protocol::Quic));
                        }
                        IpAddr::V6(ipv6) => {
                            addrs.insert(StackAddr::empty()
                                .with_protocol(Protocol::Ip6(ipv6))
                                .with_protocol(Protocol::Udp(addr.port()))
                                .with_protocol(Protocol::Quic));
                        }
                    }
                }
                TransportKind::TlsOverTcp => {
                    match addr.ip() {
                        IpAddr::V4(ipv4) => {
                            addrs.insert(StackAddr::empty()
                                .with_protocol(Protocol::Ip4(ipv4))
                                .with_protocol(Protocol::Tcp(addr.port()))
                                .with_protocol(Protocol::Tls));
                        }
                        IpAddr::V6(ipv6) => {
                            addrs.insert(StackAddr::empty()
                                .with_protocol(Protocol::Ip6(ipv6))
                                .with_protocol(Protocol::Tcp(addr.port()))
                                .with_protocol(Protocol::Tls));
                        }
                    }
                }
            }
        }
    }
    addrs
}

fn replace_with_actual_addrs(
    input_addrs: &HashSet<StackAddr>,
    protocols: &[TransportKind],
    allow_loopback: bool
) -> HashSet<StackAddr> {
    let mut result = HashSet::new();

    let actual_addrs = crate::device::get_default_server_addrs(default::DEFAULT_SERVER_PORT, allow_loopback);

    for addr in input_addrs {
        let sock_addr = match addr.socket_addr() {
            Some(sock_addr) => sock_addr,
            None => {
                tracing::error!("Invalid address: {:?}", addr);
                continue;
            }
        };
        let is_unspecified = match sock_addr.ip() {
            IpAddr::V4(ip) => ip.is_unspecified(),
            IpAddr::V6(ip) => ip.is_unspecified(),
        };

        if is_unspecified {
            for actual in &actual_addrs {
                for proto in protocols {
                    match proto {
                        TransportKind::Quic => {
                            match actual.ip() {
                                IpAddr::V4(ipv4) => {
                                    if sock_addr.ip().is_ipv4() {
                                        result.insert(StackAddr::empty()
                                            .with_protocol(Protocol::Ip4(ipv4))
                                            .with_protocol(Protocol::Udp(sock_addr.port()))
                                            .with_protocol(Protocol::Quic));
                                    }
                                }
                                IpAddr::V6(ipv6) => {
                                    if sock_addr.ip().is_ipv6() {
                                        result.insert(StackAddr::empty()
                                            .with_protocol(Protocol::Ip6(ipv6))
                                            .with_protocol(Protocol::Udp(sock_addr.port()))
                                            .with_protocol(Protocol::Quic));
                                    }
                                }
                            }
                        }
                        TransportKind::TlsOverTcp => {
                            match actual.ip() {
                                IpAddr::V4(ipv4) => {
                                    if sock_addr.ip().is_ipv4() {
                                        result.insert(StackAddr::empty()
                                            .with_protocol(Protocol::Ip4(ipv4))
                                            .with_protocol(Protocol::Tcp(sock_addr.port()))
                                            .with_protocol(Protocol::Tls));
                                    }
                                }
                                IpAddr::V6(ipv6) => {
                                    if sock_addr.ip().is_ipv6() {
                                        result.insert(StackAddr::empty()
                                            .with_protocol(Protocol::Ip6(ipv6))
                                            .with_protocol(Protocol::Tcp(sock_addr.port()))
                                            .with_protocol(Protocol::Tls));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            result.insert(addr.clone());
        }
    }
    result
}
