pub mod actor;

use actor::SocketActor;
use foctet_core::default::{
    DEFAULT_BIND_V4_ADDR, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_RETRIES, DEFAULT_RECEIVE_TIMEOUT,
    DEFAULT_SERVER_V4_ADDR, DEFAULT_WRITE_BUFFER_SIZE,
};
use foctet_core::default::{
    DEFAULT_READ_BUFFER_SIZE, DEFAULT_SEND_TIMEOUT, MAX_READ_BUFFER_SIZE, MAX_WRITE_BUFFER_SIZE,
    MIN_READ_BUFFER_SIZE, MIN_WRITE_BUFFER_SIZE,
};
use tokio_util::sync::CancellationToken;
use crate::device;
use crate::message::{AckMessage, ActorMessage, RelayActorMessage, SessionCommand};
use crate::relay::actor::RelaySocketActor;
use crate::tls::TlsConfig;

use anyhow::Result;
use anyhow::anyhow;
use foctet_core::node::{NodeAddr, NodeId};
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;
use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, SocketAddr};

/// The type of socket and transport protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketType {
    /// Both QUIC and TCP (default)
    Both,
    /// QUIC socket
    Quic,
    /// TCP socket
    Tcp,
}

impl Default for SocketType {
    fn default() -> Self {
        SocketType::Both
    }
}

/// Represents the type of connection used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
    /// Direct connection to the target node.
    Direct,
    /// Connection via a relay server.
    Relay,
}

/// Connection Information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub node_id: NodeId,
    pub socket_addr: SocketAddr,
    pub connection_type: ConnectionType,
}

impl ConnectionInfo {
    pub fn new(node_id: NodeId, socket_addr: SocketAddr, connection_type: ConnectionType) -> Self {
        Self {
            node_id,
            socket_addr,
            connection_type,
        }
    }
}

/// Determines whether the node is directly reachable or requires a relay.
fn relay_required(node_addr: &NodeAddr, include_loopback: bool) -> ConnectionType {
    let reachable_addrs = crate::connection::filter::filter_reachable_addrs(
        node_addr.socket_addresses.iter().cloned().collect(),
        include_loopback,
    );
    if reachable_addrs.is_empty() {
        return ConnectionType::Relay;
    } else {
        return ConnectionType::Direct;
    }
}

/// The configuration for the socket.
#[derive(Debug, Clone)]
pub struct SocketConfig {
    pub bind_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub socket_type: SocketType,
    pub tls_config: TlsConfig,
    pub connection_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    max_retries: usize,
    read_buffer_size: usize,
    write_buffer_size: usize,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub include_loopback: bool,
}

impl SocketConfig {
    /// Create a new socket configuration with the given TLS configuration.
    pub fn new(tls_config: TlsConfig) -> Self {
        Self {
            bind_addr: DEFAULT_BIND_V4_ADDR,
            server_addr: DEFAULT_SERVER_V4_ADDR,
            socket_type: SocketType::Quic,
            tls_config: tls_config,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_RECEIVE_TIMEOUT,
            write_timeout: DEFAULT_SEND_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            cert_path: None,
            key_path: None,
            include_loopback: false,
        }
    }
    /// Create a new socket configuration with the given TLS configuration and default bind address.
    pub fn new_with_default_addr(tls_config: TlsConfig) -> Self {
        let default_bind_addr = crate::device::get_default_bind_addr();
        Self {
            bind_addr: default_bind_addr,
            server_addr: DEFAULT_SERVER_V4_ADDR,
            socket_type: SocketType::Quic,
            tls_config: tls_config,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_RECEIVE_TIMEOUT,
            write_timeout: DEFAULT_SEND_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            cert_path: None,
            key_path: None,
            include_loopback: false,
        }
    }

    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    pub fn with_server_addr(mut self, addr: SocketAddr) -> Self {
        self.server_addr = addr;
        self
    }

    pub fn with_socket_type(mut self, socket_type: SocketType) -> Self {
        self.socket_type = socket_type;
        self
    }

    pub fn with_tls_config(mut self, config: TlsConfig) -> Self {
        self.tls_config = config;
        self
    }

    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }

    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn with_cert_path(mut self, path: PathBuf) -> Self {
        self.cert_path = Some(path);
        self
    }

    pub fn with_key_path(mut self, path: PathBuf) -> Self {
        self.key_path = Some(path);
        self
    }

    pub fn with_include_loopback(mut self, include_loopback: bool) -> Self {
        self.include_loopback = include_loopback;
        self
    }
    /// Sets a custom buffer size for sending data using builder pattern.
    pub fn with_write_buffer_size(mut self, size: usize) -> Result<Self> {
        if size < MIN_WRITE_BUFFER_SIZE || size > MAX_WRITE_BUFFER_SIZE {
            return Err(anyhow!("Write buffer size out of range"));
        }
        self.write_buffer_size = size;
        Ok(self)
    }
    /// Sets a custom buffer size for receiving data using builder pattern.
    pub fn with_read_buffer_size(mut self, size: usize) -> Result<Self> {
        if size < MIN_READ_BUFFER_SIZE || size > MAX_READ_BUFFER_SIZE {
            return Err(anyhow!("Read buffer size out of range"));
        }
        self.read_buffer_size = size;
        Ok(self)
    }
    /// Sets the write buffer size to the minimum value using builder pattern.
    pub fn with_min_write_buffer_size(mut self) -> Self {
        self.write_buffer_size = MIN_WRITE_BUFFER_SIZE;
        self
    }
    /// Sets the read buffer size to the minimum value using builder pattern.
    pub fn with_min_read_buffer_size(mut self) -> Self {
        self.read_buffer_size = MIN_READ_BUFFER_SIZE;
        self
    }
    /// Sets the write buffer size to the default value using builder pattern.
    pub fn with_default_write_buffer_size(mut self) -> Self {
        self.write_buffer_size = DEFAULT_WRITE_BUFFER_SIZE;
        self
    }
    /// Sets the read buffer size to the default value using builder pattern.
    pub fn with_default_read_buffer_size(mut self) -> Self {
        self.read_buffer_size = DEFAULT_READ_BUFFER_SIZE;
        self
    }
    /// Sets the write buffer size to the maximum value using builder pattern.
    pub fn with_max_write_buffer_size(mut self) -> Self {
        self.write_buffer_size = MAX_WRITE_BUFFER_SIZE;
        self
    }
    /// Sets the read buffer size to the maximum value using builder pattern.
    pub fn with_max_read_buffer_size(mut self) -> Self {
        self.read_buffer_size = MAX_READ_BUFFER_SIZE;
        self
    }
    /// Return the maximum number of retries.
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }
    /// Returns the write buffer size.
    pub fn write_buffer_size(&self) -> usize {
        self.write_buffer_size
    }
    /// Returns the read buffer size.
    pub fn read_buffer_size(&self) -> usize {
        self.read_buffer_size
    }
    /// Returns the server addresses.
    /// If the server address is unspecified, it returns the default server addresses.
    /// Otherwise, it returns the server address.
    /// If the server address is IPv4 unspecified, it returns the default IPv4 server addresses.
    /// If the server address is IPv6 unspecified, it returns the default server addresses, both IPv4 and IPv6 for dual-stack.
    pub fn server_addresses(&self) -> BTreeSet<SocketAddr> {
        let mut addrs = BTreeSet::new();
        match self.server_addr.ip() {
            IpAddr::V4(ipv4addr) => {
                if ipv4addr.is_unspecified() {
                    addrs = device::get_default_ipv4_server_addrs(self.include_loopback);
                } else {
                    addrs.insert(self.server_addr);
                }
            },
            IpAddr::V6(ipv6addr) => {
                if ipv6addr.is_unspecified() {
                    addrs = device::get_default_server_addrs(self.include_loopback);
                } else {
                    addrs.insert(self.server_addr);
                }
            },
        }
        addrs
    }
}

/// The socket handle.
/// It is a reference-counted pointer to the socket.
#[derive(Debug)]
pub struct SocketHandle {
    inner: Arc<RwLock<Socket>>,
}

impl Clone for SocketHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl SocketHandle {
    /// Connects to a node, deciding whether to use a relay or direct connection.
    pub async fn connect(&self, node_addr: NodeAddr) -> Result<()> {
        let mut socket = self.inner.write().await;
        match relay_required(&node_addr, socket.config.include_loopback) {
            ConnectionType::Direct => {
                let message = ActorMessage::SessionManagement(SessionCommand::Connect(node_addr));
                match socket.send_message(message).await {
                    Ok(_) => (),
                    Err(e) => return Err(e),
                }
                match socket.receive_message().await {
                    Some(message) => {
                        match message {
                            ActorMessage::Ack(ack_message) => {
                                match ack_message {
                                    AckMessage::Connected(connection_info) => {
                                        socket.connections.insert(connection_info.node_id.clone(), connection_info);
                                        Ok(())
                                    },
                                    _ => Err(anyhow!("Unexpected ack message")),
                                }
                            },
                            _ => Err(anyhow!("Unexpected message")),
                        }
                    },
                    None => Err(anyhow!("No message received")),
                }
            },
            ConnectionType::Relay => {
                if let Some(_relay_addr) = &node_addr.relay_addr {
                    let message = RelayActorMessage::SessionManagement(SessionCommand::Connect(node_addr));
                    socket.send_relay_message(message).await
                } else {
                    Err(anyhow!("Relay required, but relay address is missing"))
                }
            },
        }
    }
    pub async fn disconnect(&self, node_id: NodeId) -> Result<()> {
        let socket = self.inner.read().await;
        if let Some(connection_info) = socket.connections.get(&node_id) {
            match connection_info.connection_type {
                ConnectionType::Direct => {
                    let message = ActorMessage::SessionManagement(SessionCommand::Disconnect(node_id));
                    socket.send_message(message).await
                },
                ConnectionType::Relay => {
                    let message = RelayActorMessage::SessionManagement(SessionCommand::Disconnect(node_id));
                    socket.send_relay_message(message).await
                },
            }
        } else {
            Err(anyhow!("Connection info not found"))
        }
    }
    /// Shuts down the socket and cancels all associated actors.
    pub async fn shutdown(&self) {
        let socket = self.inner.read().await;
        socket.cancel_actor();
        socket.cancel_relay_actor();
    }
}

/// The foctet socket.
/// It can be either a QUIC socket, a TCP socket, or both.
#[derive(Debug)]
pub struct Socket {
    /// The node address of this socket.
    pub node_addr: NodeAddr,
    /// The configuration of this socket.
    pub config: SocketConfig,
    /// The actor message sender.
    actor_sender: mpsc::Sender<ActorMessage>,
    /// The actor message receiver.
    actor_receiver: mpsc::Receiver<ActorMessage>,
    /// The relay actor message sender.
    relay_actor_sender: mpsc::Sender<RelayActorMessage>,
    /// The relay actor message receiver.
    relay_actor_receiver: mpsc::Receiver<RelayActorMessage>,
    /// The incoming message receiver.
    incoming_message_receiver: mpsc::Receiver<ActorMessage>,
    /// The actor cancellation token.
    actor_cancel_token: CancellationToken,
    /// The relay actor cancellation token.
    relay_actor_cancel_token: CancellationToken,
    /// A cache to determine if a relay is required for a given NodeId.
    connections: HashMap<NodeId, ConnectionInfo>,
}

impl Socket {
    pub async fn spawn(node_addr: NodeAddr, config: SocketConfig) -> Result<SocketHandle> {
        let (from_actor_sender, from_actor_receiver) = mpsc::channel::<ActorMessage>(256);
        let (from_relay_actor_sender, from_relay_actor_receiver) = mpsc::channel::<RelayActorMessage>(256);
        let (to_actor_sender, to_actor_receiver) = mpsc::channel::<ActorMessage>(256);
        let (to_relay_actor_sender, to_relay_actor_receiver) = mpsc::channel::<RelayActorMessage>(256);
        let (incomming_message_tx, incomming_message_rx) = mpsc::channel::<ActorMessage>(256);
        
        let actor_cancel_token = CancellationToken::new();
        let cloned_actor_cancel_token = actor_cancel_token.clone();
        let relay_actor_cancel_token = CancellationToken::new();
        let cloned_relay_actor_cancel_token = relay_actor_cancel_token.clone();
        
        let socket = Self { 
            node_addr, 
            config,
            actor_sender: to_actor_sender,
            actor_receiver: from_actor_receiver,
            relay_actor_sender: to_relay_actor_sender,
            relay_actor_receiver: from_relay_actor_receiver,
            incoming_message_receiver: incomming_message_rx,
            actor_cancel_token,
            relay_actor_cancel_token,
            connections: HashMap::new(),
        };
        let socket_arc = Arc::new(RwLock::new(socket));
        let actor = SocketActor::new(Arc::clone(&socket_arc), from_actor_sender, cloned_actor_cancel_token).await?;
        // Spawn the actor
        tokio::spawn(actor.run(to_actor_receiver, incomming_message_tx));
        let relay_actor = RelaySocketActor::new(Arc::clone(&socket_arc), from_relay_actor_sender, cloned_relay_actor_cancel_token).await?;
        // Spawn the relay actor
        tokio::spawn(relay_actor.run(to_relay_actor_receiver));
        Ok(SocketHandle {
            inner: socket_arc,
        })
    }

    pub async fn send_message(&self, message: ActorMessage) -> Result<()> {
        self.actor_sender.send(message).await.map_err(|e| anyhow!(e))
    }

    pub async fn receive_message(&mut self) -> Option<ActorMessage> {
        self.actor_receiver.recv().await
    }

    pub async fn send_relay_message(&self, message: RelayActorMessage) -> Result<()> {
        self.relay_actor_sender.send(message).await.map_err(|e| anyhow!(e))
    }

    pub async fn receive_relay_message(&mut self) -> Option<RelayActorMessage> {
        self.relay_actor_receiver.recv().await
    }

    pub fn cancel_actor(&self) {
        self.actor_cancel_token.cancel();
    }

    pub fn cancel_relay_actor(&self) {
        self.relay_actor_cancel_token.cancel();
    }
}
