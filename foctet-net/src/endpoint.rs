use std::net::SocketAddr;
use std::time::Duration;

use crate::config::EndpointConfig;
use crate::config::TransportProtocol;
use crate::connection::quic::QuicConnection;
use crate::connection::quic::QuicSocket;
use crate::connection::tcp::TcpSocket;
use crate::connection::tcp::TlsTcpStream;
use crate::connection::NetworkStream;
use crate::relay::client::RelayClient;
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::node::NodeAddr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Builder for [`Endpoint`].
pub struct EndpointBuilder {
    config: EndpointConfig,
    node_addr: NodeAddr,
}

impl EndpointBuilder {
    /// Create EndpointBuilder with default values
    pub fn new() -> Self {
        Self {
            config: EndpointConfig::new(),
            node_addr: NodeAddr::unspecified(),
        }
    }
    /// Set the configuration for the endpoint.
    pub fn with_config(mut self, config: EndpointConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the node address for the endpoint.
    pub fn with_node_addr(mut self, node_addr: NodeAddr) -> Self {
        self.node_addr = node_addr;
        self
    }

    /// Set the bind address for the endpoint.
    pub fn with_bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.config = self.config.with_bind_addr(bind_addr);
        self
    }

    /// Set the server address for the endpoint.
    pub fn with_server_addr(mut self, server_addr: SocketAddr) -> Self {
        self.config = self.config.with_server_addr(server_addr);
        self
    }

    /// Set the transport protocol for the endpoint.
    pub fn with_protocol(mut self, protocol: TransportProtocol) -> Self {
        self.config = self.config.with_protocol(protocol);
        self
    }

    /// Set the connection timeout for the endpoint.
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.with_connection_timeout(timeout);
        self
    }

    /// Set the read timeout for the endpoint.
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.with_read_timeout(timeout);
        self
    }

    /// Set the write timeout for the endpoint.
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.with_write_timeout(timeout);
        self
    }

    /// Set the maximum number of retries for the endpoint.
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.config = self.config.with_max_retries(max_retries);
        self
    }

    /// Build the `Endpoint`.
    ///
    /// This initializes the underlying `QuicSocket` and `TcpSocket` based on the provided configuration.
    pub async fn build(self) -> Result<Endpoint> {
        let quic_socket = QuicSocket::new(self.node_addr.node_id.clone(), self.config.clone())?;
        let tcp_socket = TcpSocket::new(self.node_addr.node_id.clone(), self.config.clone())?;
        let relay_client = RelayClient::new(self.node_addr.clone(),self.config.clone())?;
        Ok(Endpoint {
            node_addr: self.node_addr,
            config: self.config,
            quic_socket,
            tcp_socket,
            cancellation_token: CancellationToken::new(),
            relay_client: relay_client,
        })
    }
}

/// The endpoint for the node.
pub struct Endpoint {
    /// The node address for the endpoint.
    pub node_addr: NodeAddr,
    /// The configuration for the endpoint.
    pub config: EndpointConfig,
    /// QUIC socket
    quic_socket: QuicSocket,
    /// TCP socket
    tcp_socket: TcpSocket,
    /// The cancellation token for the endpoint.
    cancellation_token: CancellationToken,
    /// The relay client for the endpoint.
    relay_client: RelayClient,
}

impl Endpoint {
    /// Create a new `EndpointBuilder` for constructing an `Endpoint`.
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder::new()
    }
    pub async fn connect(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        match self.config.transport_protocol {
            TransportProtocol::Quic => self.connect_quic_direct(node_addr).await,
            TransportProtocol::Tcp => self.connect_tcp_direct(node_addr).await,
            TransportProtocol::Both => {
                match self.connect_quic_direct(node_addr.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(_) => {}
                }
                self.connect_tcp_direct(node_addr).await
            }
        }
    }
    pub async fn connect_quic_direct(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        let mut conn = self.quic_socket.connect_node(node_addr).await?;
        let stream = conn.open_stream().await?;
        Ok(NetworkStream::Quic(stream))
    }
    pub async fn connect_quic_relay(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        let relay_addr = match &self.node_addr.relay_addr {
            Some(addr) => addr.clone(),
            None => return Err(anyhow!("Relay address not found")),
        };
        let stream = self.relay_client.connect_quic(node_addr.node_id, relay_addr).await?;
        Ok(stream)
    }
    pub async fn connect_quic(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        // 1. Try connect to the node directly
        match self.connect_quic_direct(node_addr.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                tracing::error!("Failed to connect to the node directly: {:?}", e);
            }
        }
        // 2. Try connect to the node via relay
        match self.connect_quic_relay(node_addr).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                tracing::error!("Failed to connect to the node via relay: {:?}", e);
            }
        }
        Err(anyhow!("Failed to connect to the node"))
    }
    pub async fn connect_tcp_direct(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        let stream = self.tcp_socket.connect_node(node_addr).await?;
        Ok(NetworkStream::Tcp(stream))
    }
    pub async fn connect_tcp_relay(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        let relay_addr = match &self.node_addr.relay_addr {
            Some(addr) => addr.clone(),
            None => return Err(anyhow!("Relay address not found")),
        };
        let stream = self.relay_client.connect_tcp(node_addr.node_id, relay_addr).await?;
        Ok(stream)
    }
    pub async fn connect_tcp(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        // 1. Try connect to the node directly
        match self.connect_tcp_direct(node_addr.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                tracing::error!("Failed to connect to the node directly: {:?}", e);
            }
        }
        // 2. Try connect to the node via relay
        match self.connect_tcp_relay(node_addr).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                tracing::error!("Failed to connect to the node via relay: {:?}", e);
            }
        }
        Err(anyhow!("Failed to connect to the node"))
    }
    pub async fn listen(&mut self, sender: mpsc::Sender<NetworkStream>) -> Result<()> {
        let (quic_conn_tx, mut quic_conn_rx) = mpsc::channel::<QuicConnection>(100);
        let mut quic_socket = self.quic_socket.clone();
        let quic_cancel_token = self.cancellation_token.clone();
        tokio::spawn(async move {
            match quic_socket.listen(quic_conn_tx, quic_cancel_token).await {
                Ok(_) => {
                    tracing::info!("QUIC listener stopped.");
                }
                Err(e) => {
                    tracing::error!("Error listening: {:?}", e);
                }
            }
        });
        let (tcp_conn_tx, mut tcp_conn_rx) = mpsc::channel::<TlsTcpStream>(100);
        let mut tcp_socket = self.tcp_socket.clone();
        let tcp_cancel_token = self.cancellation_token.clone();
        tokio::spawn(async move {
            match tcp_socket.listen(tcp_conn_tx, tcp_cancel_token).await {
                Ok(_) => {
                    tracing::info!("TCP listener stopped.");
                }
                Err(e) => {
                    tracing::error!("Error listening: {:?}", e);
                }
            }
        });
        loop {
            tokio::select! {
                Some(mut conn) = quic_conn_rx.recv() => {
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        match conn.accept_stream().await {
                            Ok(stream) => {
                                let stream = NetworkStream::Quic(stream);
                                if let Err(e) = sender.send(stream).await {
                                    tracing::error!("Error sending QUIC stream: {:?}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error accepting stream: {:?}", e);
                            }
                        }
                    });
                }
                Some(conn) = tcp_conn_rx.recv() => {
                    let stream = NetworkStream::Tcp(conn);
                    if let Err(e) = sender.send(stream).await {
                        tracing::error!("Error sending TCP connection: {:?}", e);
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("Endpoint listener cancelled.");
                    break;
                }
            }
        }
        Ok(())
    }
}
