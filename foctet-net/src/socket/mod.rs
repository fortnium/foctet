pub mod actor;
use foctet_core::default::{
    DEFAULT_BIND_V4_ADDR, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_RETRIES, DEFAULT_RECEIVE_TIMEOUT,
    DEFAULT_SERVER_V4_ADDR, DEFAULT_WRITE_BUFFER_SIZE,
};
use foctet_core::default::{
    DEFAULT_READ_BUFFER_SIZE, DEFAULT_SEND_TIMEOUT, MAX_READ_BUFFER_SIZE, MAX_WRITE_BUFFER_SIZE,
    MIN_READ_BUFFER_SIZE, MIN_WRITE_BUFFER_SIZE,
};
use crate::device;
use crate::tls::TlsConfig;
use crate::connection::{
    quic::{QuicConnection, QuicSocket},
    tcp::{TcpSocket, TlsTcpStream},
    FoctetStream,
};
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::{
    error::ConnectionError,
    node::{NodeAddr, NodeId},
};
use tokio::sync::mpsc;
use std::time::Duration;
use std::path::PathBuf;
use std::collections::BTreeSet;
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
                    addrs = device::get_default_ipv4_server_addrs();
                } else {
                    addrs.insert(self.server_addr);
                }
            },
            IpAddr::V6(ipv6addr) => {
                if ipv6addr.is_unspecified() {
                    addrs = device::get_default_server_addrs();
                } else {
                    addrs.insert(self.server_addr);
                }
            },
        }
        addrs
    }
}

/// The foctet socket.
/// It can be either a QUIC socket, a TCP socket, or both.
pub struct Socket {
    /// The node address of this socket.
    pub node_addr: NodeAddr,
    pub config: SocketConfig,
}

impl Socket {
    pub fn new(node_addr: NodeAddr, config: SocketConfig) -> Self {
        Self { node_addr, config }
    }
    /// Start listening for incoming connections
    pub async fn listen(&mut self) -> Result<()> {
        match self.config.socket_type {
            SocketType::Quic => {
                let node_id = self.node_addr.node_id.clone();
                let quic_config = self.config.clone();
                tokio::spawn(async move {
                    let _ = start_quic_server(node_id.clone(), quic_config).await;
                });
            }
            SocketType::Tcp => {
                let node_id = self.node_addr.node_id.clone();
                let tcp_config = self.config.clone();
                tokio::spawn(async move {
                    let _ = start_tcp_server(node_id.clone(), tcp_config).await;
                });
            }
            SocketType::Both => {
                let node_id = self.node_addr.node_id.clone();
                let quic_config = self.config.clone();
                tokio::spawn(async move {
                    let _ = start_quic_server(node_id.clone(), quic_config).await;
                });
                let node_id = self.node_addr.node_id.clone();
                let tcp_config = self.config.clone();
                tokio::spawn(async move {
                    let _ = start_tcp_server(node_id.clone(), tcp_config).await;
                });
            }
        }
        Ok(())
    }

    /// Connect to another peer
    pub async fn connect(&self, server_addr: SocketAddr, server_name: &str) -> Result<()> {
        match self.config.socket_type {
            SocketType::Quic => {
                let mut quic_socket =
                    QuicSocket::new_client(self.node_addr.node_id.clone(), self.config.clone())?;
                let _conn = quic_socket.connect(server_addr, server_name).await?;
                // Do something with the connection if needed
            }
            SocketType::Tcp => {
                let mut tcp_socket =
                    TcpSocket::new(self.node_addr.node_id.clone(), self.config.clone())?;
                let _conn = tcp_socket.connect(server_addr, server_name).await?;
                // Do something with the connection if needed
            }
            SocketType::Both => {
                let mut quic_socket =
                    QuicSocket::new_client(self.node_addr.node_id.clone(), self.config.clone())?;
                let _conn = quic_socket.connect(server_addr, server_name).await?;
                // Do something with the connection if needed
                let mut tcp_socket =
                    TcpSocket::new(self.node_addr.node_id.clone(), self.config.clone())?;
                let _conn = tcp_socket.connect(server_addr, server_name).await?;
                // Do something with the connection if needed
            }
        }
        Ok(())
    }
}

async fn start_quic_server(node_id: NodeId, config: SocketConfig) -> Result<()> {
    let mut quic_socket = QuicSocket::new(node_id, config)?;
    let (conn_tx, mut conn_rx) = mpsc::channel::<QuicConnection>(100);
    // Start the QUIC listener
    tokio::spawn(async move {
        match quic_socket.listen(conn_tx).await {
            Ok(_) => {
                tracing::info!("QUIC listener stopped.");
            }
            Err(e) => {
                tracing::error!("Error listening: {:?}", e);
            }
        }
    });
    // Handle incoming connections
    while let Some(mut conn) = conn_rx.recv().await {
        tokio::spawn(async move {
            tracing::info!("New connection: {:?}", conn.remote_address());
            loop {
                tracing::info!("Waiting for incoming stream...");
                match conn.accept_stream().await {
                    Ok(stream) => {
                        tokio::spawn(handle_stream(stream));
                    }
                    Err(e) => {
                        if let Some(stream_error) = e.downcast_ref::<ConnectionError>() {
                            match stream_error {
                                ConnectionError::Closed => {
                                    tracing::info!(
                                        "Connection closed while waiting for {}",
                                        conn.next_stream_id
                                    );
                                }
                                _ => {
                                    tracing::error!("Error accepting stream: {:?}", e);
                                }
                            }
                        } else {
                            tracing::error!("Error accepting stream: {:?}", e);
                        }
                        break;
                    }
                }
            }
        });
    }
    Ok(())
}

async fn start_tcp_server(node_id: NodeId, config: SocketConfig) -> Result<()> {
    let mut tcp_socket = TcpSocket::new(node_id, config)?;
    let (conn_tx, mut conn_rx) = mpsc::channel::<TlsTcpStream>(100);
    // Start the TCP listener
    tokio::spawn(async move {
        match tcp_socket.listen(conn_tx).await {
            Ok(_) => {
                tracing::info!("TCP listener stopped.");
            }
            Err(e) => {
                tracing::error!("Error listening: {:?}", e);
            }
        }
    });
    while let Some(conn) = conn_rx.recv().await {
        tokio::spawn(async move {
            tracing::info!("New connection: {:?}", conn.remote_address());
            tokio::spawn(handle_stream(conn));
        });
    }
    Ok(())
}

/// Handle individual streams
async fn handle_stream<S>(stream: S)
where
    S: FoctetStream + Send + 'static,
{
    let mut stream = stream;
    tracing::info!("New stream: {}", stream.stream_id());
    loop {
        match stream.receive_frame().await {
            Ok(frame) => {
                tracing::info!(
                    "{} Received frame type: {:?}",
                    stream.stream_id(),
                    frame.frame_type
                );
                tracing::info!("{} Total length: {:?}", stream.stream_id(), frame.len());
                tracing::info!(
                    "{} Payload length: {}",
                    stream.stream_id(),
                    frame.payload_len()
                );
                // Process frame (e.g., relay, respond, etc.)
            }
            Err(e) => {
                tracing::error!("Error receiving frame: {:?}", e);
                break;
            }
        }
    }
}
