use crate::{
    config::TransportConfig,
    dns::Resolver,
    transport::{
        connection::{Connection, ConnectionEvent},
        tcp::connection::TcpConnection,
        ConnectionListener, TransportHandle,
    },
};
use anyhow::{anyhow, Result};
use foctet_core::{connection::SessionId, id::NodeId};
use foctet_core::{connection::Direction, transport::ListenerId};
use foctet_mux::Session;
use stackaddr::{Protocol, StackAddr};
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_util::sync::CancellationToken;

pub struct TcpTransport {
    tls_connector: TlsConnector,
    tls_acceptor: TlsAcceptor,
    resolver: Resolver,
    pub local_node_id: NodeId,
    pub connection_timeout: Duration,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
    next_session_id: Arc<Mutex<AtomicU32>>,
}

impl TcpTransport {
    pub fn new(config: TransportConfig) -> Result<Self> {
        let connection_timeout = config.connection_timeout;
        let write_buffer_size = config.write_buffer_size;
        let read_buffer_size = config.read_buffer_size;
        let resolver = Resolver::from_option(config.dns_resolver_option.clone())?;

        // Create TLS connector and acceptor from given config.
        let tls_connector = TlsConnector::from(Arc::new(config.client_tls_config().clone()));
        let tls_acceptor = TlsAcceptor::from(Arc::new(config.server_tls_config().clone()));

        Ok(Self {
            tls_connector: tls_connector,
            tls_acceptor: tls_acceptor,
            local_node_id: config.keypair().public(),
            connection_timeout: connection_timeout,
            write_buffer_size: write_buffer_size,
            read_buffer_size: read_buffer_size,
            resolver: resolver,
            next_session_id: Arc::new(Mutex::new(AtomicU32::new(1))),
        })
    }
}

impl TransportHandle for TcpTransport {
    /// Start listening for incoming connections
    async fn listen_on(&mut self, id: ListenerId, addr: StackAddr) -> Result<ConnectionListener> {
        let mut addr = addr;
        if !addr.resolved() {
            self.resolver.resolve_addr(&mut addr).await?;
        }
        // Create a channel for sending accepted connections
        let (sender, receiver) = tokio::sync::mpsc::channel(100);

        let socket_addr = addr
            .socket_addr()
            .ok_or_else(|| anyhow!("Invalid address"))?;
        let listener = TcpListener::bind(socket_addr).await?;

        let local_socket_addr = listener.local_addr()?;
        let local_addr = StackAddr::empty()
            .with_ip(local_socket_addr.ip())
            .with_protocol(Protocol::Tcp(local_socket_addr.port()))
            .with_protocol(Protocol::Tls);
        let write_buffer_size = self.write_buffer_size;
        let read_buffer_size = self.read_buffer_size;
        let local_node_id = self.local_node_id.clone();

        let cancel_token: CancellationToken = CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();

        let tls_acceptor = self.tls_acceptor.clone();
        let session_id_clone = self.next_session_id.clone();
        // Start listening for incoming connections
        tokio::spawn(async move {
            tracing::info!("Listening on {:?}/TCP(TLS)", local_addr.socket_addr());
            loop {
                tokio::select! {
                    // Monitor the cancellation token
                    _ = cancel_token_clone.cancelled() => {
                        tracing::info!("TcpTransport listen cancelled");
                        break;
                    }
                    // Accept incoming connections
                    incoming = listener.accept() => {
                        match incoming {
                            Ok((stream, addr)) => {
                                match tls_acceptor.accept(stream).await {
                                    Ok(tls_stream) => {
                                        let session_id_lock = session_id_clone.lock().await;
                                        let session_id = SessionId::new(session_id_lock.fetch_add(1, Ordering::SeqCst));
                                        drop(session_id_lock);
                                        // Check peer certificate
                                        let remote_node_id = match extract_server_remote_node_id(&tls_stream) {
                                            Ok(node_id) => node_id,
                                            Err(e) => {
                                                tracing::error!("Failed to extract remote node ID: {:?}", e);
                                                return;
                                            }
                                        };
                                        tracing::debug!("Remote Node ID: {:?}", remote_node_id.to_bytes());

                                        let mut connection = Session::new_server(TlsStream::Server(tls_stream), session_id).await;
                                        connection.set_local_addr(local_socket_addr);
                                        connection.set_remote_addr(addr);

                                        let remote_addr = StackAddr::empty().with_ip(addr.ip()).with_protocol(Protocol::Tcp(addr.port())).with_protocol(Protocol::Tls);
                                        tracing::info!("Accepted connection from {}", addr);

                                        let tcp_conn = TcpConnection::new(
                                            Direction::Incoming,
                                            local_addr.clone(),
                                            remote_addr,
                                            connection,
                                        ).with_write_buffer_size(write_buffer_size)
                                         .with_read_buffer_size(read_buffer_size)
                                         .with_local_node_id(local_node_id)
                                         .with_remote_node_id(remote_node_id);
                                        if sender.send(ConnectionEvent::Accepted(Connection::Tcp(tcp_conn))).await.is_err() {
                                            tracing::warn!("Failed to send TcpConnection to the channel");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to complete TLS handshake: {:?}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error accepting TCP connection: {:?}", e);
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
        let server_socket_addr = addr
            .socket_addr()
            .ok_or(anyhow::anyhow!("Invalid address"))?;
        let name =
            rustls_pki_types::ServerName::try_from(addr.name().unwrap_or("localhost").to_string())?;
        let stream = match tokio::time::timeout(
            self.connection_timeout,
            TcpStream::connect(server_socket_addr),
        )
        .await
        {
            Ok(connect_result) => connect_result?,
            Err(elapsed) => return Err(anyhow!("TCP Connection timed out after {:?}", elapsed)),
        };
        let local_socket_addr = stream.local_addr()?;
        let local_addr = StackAddr::empty()
            .with_ip(local_socket_addr.ip())
            .with_protocol(Protocol::Tcp(local_socket_addr.port()))
            .with_protocol(Protocol::Tls);
        let tls_stream = match tokio::time::timeout(
            self.connection_timeout,
            self.tls_connector.connect(name, stream),
        )
        .await
        {
            Ok(connect_result) => connect_result?,
            Err(elapsed) => return Err(anyhow!("TLS Connection timed out after {:?}", elapsed)),
        };

        // Check peer certificate
        let remote_node_id = match extract_client_remote_node_id(&tls_stream) {
            Ok(node_id) => node_id,
            Err(e) => {
                tracing::error!("Failed to extract remote node ID: {:?}", e);
                return Err(e);
            }
        };
        tracing::debug!("Remote Node ID: {:?}", remote_node_id.to_bytes());

        let session_id_lock = self.next_session_id.lock().await;
        let session_id = SessionId::new(session_id_lock.fetch_add(1, Ordering::SeqCst));
        drop(session_id_lock);
        let mut connection = Session::new_client(
            TlsStream::Client(tls_stream),
            session_id,
        )
        .await;
        connection.set_local_addr(local_socket_addr);
        connection.set_remote_addr(server_socket_addr);

        let tcp_connection = TcpConnection::new(
            Direction::Outgoing,
            local_addr,
            addr,
            connection,
        ).with_write_buffer_size(self.write_buffer_size)
        .with_read_buffer_size(self.read_buffer_size)
        .with_local_node_id(self.local_node_id)
        .with_remote_node_id(remote_node_id);
        Ok(Connection::Tcp(tcp_connection))
    }
}

/// Extract the remote node ID from the server TLS stream
fn extract_server_remote_node_id(tls_stream: &tokio_rustls::server::TlsStream<tokio::net::TcpStream>) -> Result<NodeId> {
    // Check peer certificate
    let certs = tls_stream.get_ref().1.peer_certificates()
        .ok_or(anyhow!("No peer identity"))?;
    let cert = certs.get(0).ok_or(anyhow!("No certificate found"))?;
    // Parse the certificate
    let foctet_cert = crate::tls::cert::parse(cert)?;
    Ok(foctet_cert.node_id())
}

/// Extract the remote node ID from the client TLS stream
fn extract_client_remote_node_id(tls_stream: &tokio_rustls::client::TlsStream<tokio::net::TcpStream>) -> Result<NodeId> {
    // Check peer certificate
    let certs = tls_stream.get_ref().1.peer_certificates()
        .ok_or(anyhow!("No peer identity"))?;
    let cert = certs.get(0).ok_or(anyhow!("No certificate found"))?;
    // Parse the certificate
    let foctet_cert = crate::tls::cert::parse(cert)?;
    Ok(foctet_cert.node_id())
}
