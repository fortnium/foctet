use std::net::SocketAddr;
use std::time::Duration;
use std::path::PathBuf;
use std::collections::BTreeSet;
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::default::{
    DEFAULT_BIND_V4_ADDR, DEFAULT_BIND_V6_ADDR, DEFAULT_SERVER_V4_ADDR, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_RETRIES, DEFAULT_RECEIVE_TIMEOUT, DEFAULT_WRITE_BUFFER_SIZE
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

/// The configuration for the socket.
#[derive(Debug, Clone)]
pub struct EndpointConfig {
    pub bind_addr: SocketAddr,
    pub default_server_addr: SocketAddr,
    pub server_addresses: BTreeSet<SocketAddr>,
    pub transport_protocol: TransportProtocol,
    pub connection_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_retries: usize,
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub subject_alt_names: Vec<String>,
    pub insecure: bool,
    pub include_loopback: bool,
}

impl EndpointConfig {
    /// Create a new socket configuration with the default values.
    pub fn new() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_V4_ADDR,
            default_server_addr: DEFAULT_SERVER_V4_ADDR,
            server_addresses: BTreeSet::new(),
            transport_protocol: TransportProtocol::Both,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_RECEIVE_TIMEOUT,
            write_timeout: DEFAULT_SEND_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            cert_path: None,
            key_path: None,
            subject_alt_names: Vec::new(),
            insecure: false,
            include_loopback: false,
        }
    }
    /// Create a new socket configuration with the default bind address.
    pub fn new_with_default_addr() -> Self {
        let default_bind_addr = device::get_default_bind_addr();
        let default_server_addrs = device::get_default_server_addrs(false);
        Self {
            bind_addr: default_bind_addr,
            default_server_addr: DEFAULT_SERVER_V4_ADDR,
            server_addresses: default_server_addrs,
            transport_protocol: TransportProtocol::Both,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_RECEIVE_TIMEOUT,
            write_timeout: DEFAULT_SEND_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            cert_path: None,
            key_path: None,
            subject_alt_names: Vec::new(),
            insecure: false,
            include_loopback: false,
        }
    }

    pub fn with_bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    pub fn with_server_addr(mut self, addr: SocketAddr) -> Self {
        self.default_server_addr = addr;
        if addr == DEFAULT_BIND_V4_ADDR {
            self.server_addresses = device::get_default_ipv4_server_addrs(self.include_loopback);
        } else if addr == DEFAULT_BIND_V6_ADDR {
            self.server_addresses = device::get_default_server_addrs(self.include_loopback);
        } else {
            self.server_addresses.insert(addr);
        }
        self
    }

    pub fn with_default_v4_server_addr(mut self) -> Self {
        self.server_addresses = device::get_default_ipv4_server_addrs(self.include_loopback);
        self
    }

    pub fn with_default_v6_server_addr(mut self) -> Self {
        self.server_addresses = device::get_default_server_addrs(self.include_loopback);
        self
    }

    pub fn with_protocol(mut self, transport_protocol: TransportProtocol) -> Self {
        self.transport_protocol = transport_protocol;
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

    pub fn with_cert_path_option(mut self, path_option: Option<PathBuf>) -> Self {
        self.cert_path = path_option;
        self
    }

    pub fn with_key_path(mut self, path: PathBuf) -> Self {
        self.key_path = Some(path);
        self
    }

    pub fn with_key_path_option(mut self, path_option: Option<PathBuf>) -> Self {
        self.key_path = path_option;
        self
    }

    pub fn with_subject_alt_names(mut self, names: Vec<String>) -> Self {
        self.subject_alt_names = names;
        self
    }

    pub fn with_subject_alt_name(mut self, name: String) -> Self {
        if !self.subject_alt_names.contains(&name) {
            self.subject_alt_names.push(name);
        }
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
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
    /// Retunrs the default server address.
    pub fn server_addr(&self) -> SocketAddr {
        self.default_server_addr
    }
    /// Returns server addresses.
    pub fn server_addresses(&self) -> BTreeSet<SocketAddr> {
        self.server_addresses.clone()
    }
    pub fn tls_client_config(&self) -> Result<TlsClientConfig> {
        if self.insecure {
            TlsClientConfigBuilder::new_insecure()
        } else {
            TlsClientConfigBuilder::new_with_native_certs()
        }
    }
    pub fn tls_server_config(&self) -> Result<TlsServerConfig> {
        if self.insecure {
            TlsServerConfigBuilder::new_insecure(self.subject_alt_names.clone())
        } else {
            if self.cert_path.is_some() && self.key_path.is_some() {
                TlsServerConfigBuilder::new_with_cert(&self.cert_path.as_ref().unwrap(), &self.key_path.as_ref().unwrap())
            } else {
                Err(anyhow!("Certificate and key paths are not set"))
            }
        }
    }
}
