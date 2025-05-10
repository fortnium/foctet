use anyhow::{anyhow, Result};
use foctet_core::{
    connection::Direction, default::{DEFAULT_BIND_V4_ADDR, DEFAULT_BIND_V6_ADDR}, id::NodeId, transport::ListenerId
};
use quinn::Endpoint;
use stackaddr::{Protocol, StackAddr};
use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    sync::Arc,
    time::Duration,
};
use tokio_util::sync::CancellationToken;

use crate::{
    config::TransportConfig, dns::Resolver, transport::{
        connection::{Connection, ConnectionEvent},
        quic::connection::QuicConnection,
        ConnectionListener, TransportHandle,
    }
};

use super::config::{QuicConfig, QuinnConfig};

pub struct QuicTransport {
    /// Config for the inner [`quinn`] structs.
    quinn_config: QuinnConfig,
    resolver: Resolver,
    pub local_node_id: NodeId,
    pub connection_timeout: Duration,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
}

impl QuicTransport {
    pub fn new(config: TransportConfig) -> Result<Self> {
        let connection_timeout = config.connection_timeout;
        let write_buffer_size = config.write_buffer_size;
        let read_buffer_size = config.read_buffer_size;
        let resolver = Resolver::from_option(config.dns_resolver_option.clone())?;
        let local_node_id = config.keypair().public();
        // Create quinn config from given config.
        let quic_config: QuicConfig = config.into();
        let quinn_config: QuinnConfig = quic_config.into();

        Ok(Self {
            quinn_config: quinn_config,
            connection_timeout: connection_timeout,
            local_node_id: local_node_id,
            write_buffer_size: write_buffer_size,
            read_buffer_size: read_buffer_size,
            resolver: resolver,
        })
    }
}

impl TransportHandle for QuicTransport {
    /// Start listening for incoming connections
    async fn listen_on(&mut self, id: ListenerId, addr: StackAddr) -> Result<ConnectionListener> {
        let mut addr = addr;
        if !addr.resolved() {
            self.resolver.resolve_addr(&mut addr).await?;
        }
        // Create a channel for sending accepted connections
        let (sender, receiver) = tokio::sync::mpsc::channel(100);

        let runtime = Arc::new(quinn::TokioRuntime);

        let socket = match addr.socket_addr() {
            Some(socket_addr) => {
                let socket = UdpSocket::bind(socket_addr)
                    .map_err(|e| anyhow!("Failed to bind UDP socket: {:?}", e))?;
                socket
            }
            None => return Err(anyhow!("Invalid address")),
        };

        let mut endpoint = Endpoint::new(
            self.quinn_config.endpoint_config.clone(),
            Some(self.quinn_config.server_config.clone()),
            socket,
            runtime,
        )?;
        endpoint.set_default_client_config(self.quinn_config.client_config.clone());

        let local_socket_addr = endpoint.local_addr()?;
        let local_addr = StackAddr::empty()
            .with_ip(local_socket_addr.ip())
            .with_protocol(Protocol::Udp(local_socket_addr.port()))
            .with_protocol(Protocol::Quic);
        let write_buffer_size = self.write_buffer_size;
        let read_buffer_size = self.read_buffer_size;
        let local_node_id = self.local_node_id.clone();

        let cancel_token: CancellationToken = CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();

        // Start listening for incoming connections
        tokio::spawn(async move {
            tracing::info!("Listening on {:?}/UDP(QUIC)", local_addr.socket_addr());
            loop {
                tokio::select! {
                    // Monitor the cancellation token
                    _ = cancel_token_clone.cancelled() => {
                        tracing::info!("QuicTransport listen cancelled");
                        break;
                    }
                    // Accept incoming connections
                    incoming = endpoint.accept() => {
                        match incoming {
                            Some(incoming_connection) => {
                                match incoming_connection.await {
                                    Ok(connection) => {
                                        let remote_socket_addr = connection.remote_address();
                                        let remote_addr = StackAddr::empty().with_ip(remote_socket_addr.ip()).with_protocol(Protocol::Udp(remote_socket_addr.port())).with_protocol(Protocol::Quic);
                                        tracing::info!("Accepted connection from {}", connection.remote_address());

                                        // Check peer certificate
                                        let remote_node_id = match extract_remote_node_id(&connection) {
                                            Ok(node_id) => node_id,
                                            Err(e) => {
                                                tracing::error!("Failed to extract remote node ID: {:?}", e);
                                                return;
                                            }
                                        };
                                        tracing::debug!("Remote Node ID: {:?}", remote_node_id.to_bytes());
                                        // Create a new QuicConnection
                                        let quic_conn = QuicConnection::new(
                                            Direction::Incoming,
                                            local_addr.clone(),
                                            remote_addr,
                                            connection,
                                        ).with_write_buffer_size(write_buffer_size)
                                        .with_read_buffer_size(read_buffer_size)
                                        .with_local_node_id(local_node_id)
                                        .with_remote_node_id(remote_node_id);
                                        if sender.send(ConnectionEvent::Accepted(Connection::Quic(quic_conn))).await.is_err() {
                                            tracing::warn!("Failed to send QuicConnection to the channel");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Error accepting connection: {:?}", e);
                                    }
                                }
                            }
                            None => {
                                tracing::warn!("No incoming connection; endpoint may have been closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(ConnectionListener::new(id, receiver, cancel_token))
    }

    /// Connect to a remote node
    async fn connect(&mut self, addr: StackAddr) -> Result<Connection> {
        let mut addr = addr;
        if !addr.resolved() {
            self.resolver.resolve_addr(&mut addr).await?;
        }
        let runtime = Arc::new(quinn::TokioRuntime);
        let remote_ip = addr.ip().ok_or(anyhow!("Invalid address"))?;
        let bind_addr: SocketAddr = match remote_ip {
            IpAddr::V4(_) => DEFAULT_BIND_V4_ADDR,
            IpAddr::V6(_) => DEFAULT_BIND_V6_ADDR,
        };
        let socket = UdpSocket::bind(bind_addr)?;
        let mut endpoint = Endpoint::new(
            self.quinn_config.endpoint_config.clone(),
            None,
            socket,
            runtime,
        )?;
        endpoint.set_default_client_config(self.quinn_config.client_config.clone());

        let local_socket_addr = endpoint.local_addr()?;
        let local_addr = StackAddr::empty()
            .with_ip(local_socket_addr.ip())
            .with_protocol(Protocol::Udp(local_socket_addr.port()))
            .with_protocol(Protocol::Quic);

        let socket_addr = addr
            .socket_addr()
            .ok_or(anyhow::anyhow!("Invalid address"))?;
        let connecting = endpoint.connect(socket_addr, addr.name().unwrap_or("localhost"))?;
        let connection = match tokio::time::timeout(self.connection_timeout, connecting).await {
            Ok(connect_result) => connect_result?,
            Err(elapsed) => return Err(anyhow!("QUIC Connection timed out after {:?}", elapsed)),
        };
        tracing::info!("Connected to {:?}", connection.remote_address());

        // Check peer certificate
        let remote_node_id = match extract_remote_node_id(&connection) {
            Ok(node_id) => node_id,
            Err(e) => {
                tracing::error!("Failed to extract remote node ID: {:?}", e);
                return Err(e);
            }
        };
        tracing::debug!("Remote Node ID: {:?}", remote_node_id.to_bytes());
        let quic_conn: QuicConnection = QuicConnection::new(
            Direction::Outgoing,
            local_addr,
            addr,
            connection,
        ).with_write_buffer_size(self.write_buffer_size)
        .with_read_buffer_size(self.read_buffer_size)
        .with_local_node_id(self.local_node_id)
        .with_remote_node_id(remote_node_id);
        Ok(Connection::Quic(quic_conn))
    }
}

fn extract_remote_node_id(
    connection: &quinn::Connection,
) -> Result<NodeId> {
    let peer_identity = connection.peer_identity()
        .ok_or(anyhow!("No peer identity"))?;
    // try downcasting to Vec<rustls::pki_types::CertificateDer>
    let certs = peer_identity.downcast_ref::<Vec<rustls::pki_types::CertificateDer>>()
        .ok_or(anyhow!("Failed to downcast peer identity"))?;
    // Parse the certificate
    let cert = certs.get(0).ok_or(anyhow!("No certificate found"))?;
    let foctet_cert = crate::tls::cert::parse(cert)?;
    Ok(foctet_cert.node_id())
}
