use crate::socket::SocketType;
use crate::tls::TlsConfig;
use anyhow::anyhow;
use anyhow::Result;
use foctet_core::default::{
    DEFAULT_BIND_V4_ADDR, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_RETRIES, DEFAULT_RECEIVE_TIMEOUT,
    DEFAULT_SERVER_V4_ADDR, DEFAULT_WRITE_BUFFER_SIZE,
};
use foctet_core::default::{
    DEFAULT_READ_BUFFER_SIZE, DEFAULT_SEND_TIMEOUT, MAX_READ_BUFFER_SIZE, MAX_WRITE_BUFFER_SIZE,
    MIN_READ_BUFFER_SIZE, MIN_WRITE_BUFFER_SIZE,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

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
}
