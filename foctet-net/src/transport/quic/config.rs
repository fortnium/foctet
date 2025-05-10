use std::{sync::Arc, time::Duration};

use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    MtuDiscoveryConfig, VarInt,
};

use crate::{config::TransportConfig, tls};

/// Config for the QUIC transport.
#[derive(Clone)]
pub struct QuicConfig {
    /// Timeout for the initial handshake when establishing a connection.
    /// The actual timeout is the minimum of this and the [`Config::max_idle_timeout`].
    pub handshake_timeout: Duration,
    /// Maximum duration of inactivity in ms to accept before timing out the connection.
    pub max_idle_timeout: u32,
    /// Period of inactivity before sending a keep-alive packet.
    /// Must be set lower than the idle_timeout of both
    /// peers to be effective.
    ///
    /// See [`quinn::TransportConfig::keep_alive_interval`] for more
    /// info.
    pub keep_alive_interval: Duration,
    /// Maximum number of incoming bidirectional streams that may be open
    /// concurrently by the remote peer.
    pub max_concurrent_stream_limit: u32,

    /// Max unacknowledged data in bytes that may be sent on a single stream.
    pub max_stream_data: u32,

    /// Max unacknowledged data in bytes that may be sent in total on all streams
    /// of a connection.
    pub max_connection_data: u32,

    /// TLS client config for the inner [`quinn::ClientConfig`].
    client_tls_config: Arc<QuicClientConfig>,
    /// TLS server config for the inner [`quinn::ServerConfig`].
    server_tls_config: Arc<QuicServerConfig>,
    /// Foctet identity of the node.
    keypair: foctet_core::key::Keypair,

    /// Parameters governing MTU discovery. See [`MtuDiscoveryConfig`] for details.
    mtu_discovery_config: Option<MtuDiscoveryConfig>,
}

impl QuicConfig {
    /// Creates a new configuration object with default values.
    pub fn new(keypair: &foctet_core::key::Keypair) -> Self {
        let client_tls_config = Arc::new(
            QuicClientConfig::try_from(tls::config::make_client_config(keypair, None).unwrap())
                .unwrap(),
        );
        let server_tls_config = Arc::new(
            QuicServerConfig::try_from(tls::config::make_server_config(keypair).unwrap()).unwrap(),
        );
        Self {
            client_tls_config,
            server_tls_config,
            handshake_timeout: Duration::from_secs(5),
            max_idle_timeout: 10 * 1000,
            max_concurrent_stream_limit: 256,
            max_connection_data: 15_000_000,
            max_stream_data: 10_000_000,
            keep_alive_interval: Duration::from_secs(5),
            keypair: keypair.clone(),
            mtu_discovery_config: Some(Default::default()),
        }
    }
    /// Set the upper bound to the max UDP payload size that MTU discovery will search for.
    pub fn set_mtu_upper_bound(&mut self, value: u16) {
        self.mtu_discovery_config
            .get_or_insert_with(Default::default)
            .upper_bound(value);
    }

    /// Disable MTU path discovery (it is enabled by default).
    pub fn disable_path_mtu_discovery(&mut self) {
        self.mtu_discovery_config = None;
    }
}

impl From<TransportConfig> for QuicConfig {
    fn from(config: TransportConfig) -> QuicConfig {
        let client_tls_config =
            Arc::new(QuicClientConfig::try_from(config.client_tls_config().clone()).unwrap());
        let server_tls_config =
            Arc::new(QuicServerConfig::try_from(config.server_tls_config().clone()).unwrap());
        let mut quic_config = QuicConfig {
            client_tls_config,
            server_tls_config,
            handshake_timeout: config.connection_timeout,
            max_idle_timeout: 10 * 1000,
            max_concurrent_stream_limit: config.max_concurrent_stream,
            max_connection_data: 15_000_000,
            max_stream_data: 10_000_000,
            keep_alive_interval: Duration::from_secs(5),
            keypair: config.keypair().clone(),
            mtu_discovery_config: Some(Default::default()),
        };
        if let Some(mtu_upper_bound) = config.mtu_upper_bound {
            quic_config.set_mtu_upper_bound(mtu_upper_bound);
        }
        if let Some(enable_mtu_discovery) = config.enable_mtu_discovery {
            if !enable_mtu_discovery {
                quic_config.disable_path_mtu_discovery();
            }
        }
        quic_config
    }
}

/// Represents the inner configuration for [`quinn`].
#[derive(Debug, Clone)]
pub(crate) struct QuinnConfig {
    pub(crate) client_config: quinn::ClientConfig,
    pub(crate) server_config: quinn::ServerConfig,
    pub(crate) endpoint_config: quinn::EndpointConfig,
}

impl From<QuicConfig> for QuinnConfig {
    fn from(config: QuicConfig) -> QuinnConfig {
        let QuicConfig {
            client_tls_config,
            server_tls_config,
            max_idle_timeout,
            max_concurrent_stream_limit,
            keep_alive_interval,
            max_connection_data,
            max_stream_data,
            handshake_timeout: _,
            keypair,
            mtu_discovery_config,
        } = config;
        let mut transport = quinn::TransportConfig::default();
        // Disable uni-directional streams.
        transport.max_concurrent_uni_streams(0u32.into());
        transport.max_concurrent_bidi_streams(max_concurrent_stream_limit.into());
        // Disable datagrams.
        transport.datagram_receive_buffer_size(None);
        transport.keep_alive_interval(Some(keep_alive_interval));
        transport.max_idle_timeout(Some(VarInt::from_u32(max_idle_timeout).into()));
        transport.allow_spin(false);
        transport.stream_receive_window(max_stream_data.into());
        transport.receive_window(max_connection_data.into());
        transport.mtu_discovery_config(mtu_discovery_config);
        let transport = Arc::new(transport);

        let mut server_config = quinn::ServerConfig::with_crypto(server_tls_config);
        server_config.transport = Arc::clone(&transport);
        // Disables connection migration.
        // TODO!: Need to handle address change in the `Connection`.
        server_config.migration(false);

        let mut client_config = quinn::ClientConfig::new(client_tls_config);
        client_config.transport_config(transport);

        let secret = keypair.secret().to_bytes();
        let reset_key = Arc::new(ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &secret));
        let endpoint_config = quinn::EndpointConfig::new(reset_key);

        QuinnConfig {
            client_config,
            server_config,
            endpoint_config,
        }
    }
}
