use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use std::path::PathBuf;
use std::collections::BTreeSet;
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::default::{
    DEFAULT_BIND_V4_ADDR, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_RETRIES, DEFAULT_RECEIVE_TIMEOUT,
    DEFAULT_SERVER_V4_ADDR, DEFAULT_WRITE_BUFFER_SIZE,
};
use foctet_core::default::{
    DEFAULT_READ_BUFFER_SIZE, DEFAULT_SEND_TIMEOUT, MAX_READ_BUFFER_SIZE, MAX_WRITE_BUFFER_SIZE,
    MIN_READ_BUFFER_SIZE, MIN_WRITE_BUFFER_SIZE,
};
use crate::device;
use crate::tls::{TlsClientConfig, TlsClientConfigBuilder, TlsServerConfig, TlsServerConfigBuilder};

/// The type of socket and transport protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportProtocol {
    /// Both QUIC and TCP (default)
    Both,
    /// QUIC socket
    Quic,
    /// TCP socket
    Tcp,
}

pub enum EndpointConfig {
    Client(ClientConfig),
    Server(ServerConfig),
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub transport_protocol: TransportProtocol,
    pub tls_config: TlsServerConfig,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_retries: usize,
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub include_loopback: bool,
}

impl ServerConfig {
    /// Create a new server configuration with the given TLS configuration.
    pub fn new(tls_config: TlsServerConfig) -> Self {
        Self {
            bind_addr: DEFAULT_SERVER_V4_ADDR,
            transport_protocol: TransportProtocol::Both,
            tls_config: tls_config,
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
    pub fn with_transport_protocol(mut self, transport_protocol: TransportProtocol) -> Self {
        self.transport_protocol = transport_protocol;
        self
    }
    pub fn with_tls_config(mut self, config: TlsServerConfig) -> Self {
        self.tls_config = config;
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
    /// Returns the server addresses.
    /// If the server address is unspecified, it returns the default server addresses.
    /// Otherwise, it returns the server address.
    /// If the server address is IPv4 unspecified, it returns the default IPv4 server addresses.
    /// If the server address is IPv6 unspecified, it returns the default server addresses, both IPv4 and IPv6 for dual-stack.
    pub fn server_addresses(&self) -> BTreeSet<SocketAddr> {
        let mut addrs = BTreeSet::new();
        match self.bind_addr.ip() {
            IpAddr::V4(ipv4addr) => {
                if ipv4addr.is_unspecified() {
                    addrs = device::get_default_ipv4_server_addrs(self.include_loopback);
                } else {
                    addrs.insert(self.bind_addr);
                }
            },
            IpAddr::V6(ipv6addr) => {
                if ipv6addr.is_unspecified() {
                    addrs = device::get_default_server_addrs(self.include_loopback);
                } else {
                    addrs.insert(self.bind_addr);
                }
            },
        }
        addrs
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(TlsServerConfigBuilder::new_insecure(vec!["localhost".to_string()]).unwrap())
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub bind_addr: SocketAddr,
    pub transport_protocol: TransportProtocol,
    pub tls_config: TlsClientConfig,
    pub connection_timeout: Duration,
    pub read_timeout: Duration,
    pub max_retries: usize,
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub write_timeout: Duration,
    pub include_loopback: bool,
}

impl ClientConfig {
    /// Create a new client configuration with the given TLS configuration.
    pub fn new(tls_config: TlsClientConfig) -> Self {
        Self {
            bind_addr: DEFAULT_BIND_V4_ADDR,
            transport_protocol: TransportProtocol::Both,
            tls_config: tls_config,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_RECEIVE_TIMEOUT,
            write_timeout: DEFAULT_SEND_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            include_loopback: false,
        }
    }
    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }
    pub fn with_transport_protocol(mut self, transport_protocol: TransportProtocol) -> Self {
        self.transport_protocol = transport_protocol;
        self
    }
    pub fn with_tls_config(mut self, config: TlsClientConfig) -> Self {
        self.tls_config = config;
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
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::new(TlsClientConfigBuilder::new_insecure().unwrap())
    }
}
