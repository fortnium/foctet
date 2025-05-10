use crate::dns::ResolverOption;
use crate::tls;
use anyhow::Result;
use foctet_core::default::DEFAULT_CONNECTION_TIMEOUT;
use foctet_core::default::DEFAULT_KEEP_ALIVE_INTERVAL;
use foctet_core::default::DEFAULT_MAX_CONCURRENT_STREAM;
use foctet_core::default::DEFAULT_READ_BUFFER_SIZE;
use foctet_core::default::DEFAULT_READ_TIMEOUT;
use foctet_core::default::DEFAULT_WRITE_BUFFER_SIZE;
use foctet_core::default::DEFAULT_WRITE_TIMEOUT;
use std::time::Duration;

/// The configuration for the transport.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub connection_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub mtu_upper_bound: Option<u16>,
    pub enable_mtu_discovery: Option<bool>,
    pub nodelay: Option<bool>,
    pub max_concurrent_stream: u32,
    pub keep_alive_interval: Option<Duration>,
    pub dns_resolver_option: ResolverOption,
    client_tls_config: rustls::ClientConfig,
    server_tls_config: rustls::ServerConfig,
    keypair: foctet_core::key::Keypair,
}

impl TransportConfig {
    /// Creates a new configuration with default values.
    pub fn new(keypair: foctet_core::key::Keypair) -> Result<Self> {
        let client_tls_config = tls::config::make_client_config(&keypair, None)?;
        let server_tls_config = tls::config::make_server_config(&keypair)?;
        Ok(Self {
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
            mtu_upper_bound: None,
            enable_mtu_discovery: None,
            nodelay: None,
            max_concurrent_stream: DEFAULT_MAX_CONCURRENT_STREAM,
            keep_alive_interval: Some(DEFAULT_KEEP_ALIVE_INTERVAL),
            dns_resolver_option: ResolverOption::Default,
            client_tls_config: client_tls_config,
            server_tls_config: server_tls_config,
            keypair: keypair,
        })
    }
    pub fn client_tls_config(&self) -> &rustls::ClientConfig {
        &self.client_tls_config
    }
    pub fn server_tls_config(&self) -> &rustls::ServerConfig {
        &self.server_tls_config
    }
    pub fn keypair(&self) -> &foctet_core::key::Keypair {
        &self.keypair
    }
    pub fn set_keypair(&mut self, keypair: foctet_core::key::Keypair) -> Result<()> {
        let client_tls_config = tls::config::make_client_config(&keypair, None)?;
        let server_tls_config = tls::config::make_server_config(&keypair)?;
        self.client_tls_config = client_tls_config;
        self.server_tls_config = server_tls_config;
        self.keypair = keypair;
        Ok(())
    }
}
