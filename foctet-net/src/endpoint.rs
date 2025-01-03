use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::EndpointConfig;
use crate::config::TransportProtocol;
use crate::connection::quic::QuicConnection;
use crate::connection::quic::QuicSocket;
use crate::connection::tcp::TcpSocket;
use crate::connection::tcp::TlsTcpStream;
use crate::connection::FoctetStream;
use crate::connection::NetworkStream;
use crate::relay::client::RelayClient;
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::error::StreamError;
use foctet_core::frame::FrameType;
use foctet_core::frame::Payload;
use foctet_core::node::NodeAddr;
use foctet_core::node::NodeId;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// A listener for incoming network streams.
pub struct Listener {
    receiver: mpsc::Receiver<NetworkStream>,
}

impl Listener {
    /// Accept a new (next) stream
    pub async fn accept(&mut self) -> Option<NetworkStream> {
        self.receiver.recv().await
    }
}

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

    pub fn with_cert_path(mut self, cert_path: PathBuf) -> Self {
        self.config = self.config.with_cert_path(cert_path);
        self
    }

    pub fn with_cert_path_option(mut self, cert_path_option: Option<PathBuf>) -> Self {
        self.config = self.config.with_cert_path_option(cert_path_option);
        self
    }

    pub fn with_key_path(mut self, key_path: PathBuf) -> Self {
        self.config = self.config.with_key_path(key_path);
        self
    }

    pub fn with_key_path_option(mut self, key_path_option: Option<PathBuf>) -> Self {
        self.config = self.config.with_key_path_option(key_path_option);
        self
    }

    pub fn with_subject_alt_names(mut self, subject_alt_names: Vec<String>) -> Self {
        self.config = self.config.with_subject_alt_names(subject_alt_names);
        self
    }

    pub fn with_subject_alt_name(mut self, subject_alt_name: String) -> Self {
        self.config = self.config.with_subject_alt_name(subject_alt_name);
        self
    }

    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.config = self.config.with_insecure(insecure);
        self
    }

    pub fn with_include_loopback(mut self, include_loopback: bool) -> Self {
        self.config = self.config.with_include_loopback(include_loopback);
        self
    }

    /// Build the `Endpoint`.
    ///
    /// This initializes the underlying `QuicSocket` and `TcpSocket` based on the provided configuration.
    pub async fn build(self) -> Result<Endpoint> {
        tracing::info!("Building Endpoint...");
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
            quic_connections: Mutex::new(HashMap::new()),
        })
    }
}

#[derive(Debug)]
pub struct EndpointHandle {
    cancel: CancellationToken,
}

impl EndpointHandle {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
        }
    }
    pub async fn shutdown(&self) {
        self.cancel.cancel();
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
    /// Active QUIC connections
    quic_connections: Mutex<HashMap<NodeId, QuicConnection>>,
}

impl Endpoint {
    /// Create a new `EndpointBuilder` for constructing an `Endpoint`.
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder::new()
    }
    pub fn handle(&self) -> EndpointHandle {
        EndpointHandle::new(self.cancellation_token.clone())
    }
    pub async fn connect(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        match self.config.transport_protocol {
            TransportProtocol::Quic => self.connect_quic_direct(node_addr).await,
            TransportProtocol::Tcp => self.connect_tcp_direct(node_addr).await,
            TransportProtocol::Both => {
                match self.connect_quic_direct(node_addr.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(e) => {
                        tracing::error!("Failed to connect to the node directly: {:?}", e);
                    }
                }
                self.connect_tcp_direct(node_addr).await
            }
        }
    }
    pub async fn connect_quic_direct(&mut self, node_addr: NodeAddr) -> Result<NetworkStream> {
        let mut connections = self.quic_connections.lock().await;

        // Check if a connection already exists
        let node_id = node_addr.node_id.clone();
        if let Some(conn) = connections.get_mut(&node_id) {
            // Use the existing connection to open a new stream
            tracing::info!("Reusing existing QUIC connection to {:?}", node_addr);
            let stream = conn.open_stream().await?;
            return Ok(NetworkStream::Quic(stream));
        }

        let mut conn = self.quic_socket.connect_node(node_addr).await?;
        let mut stream = conn.open_stream().await?;
        stream.handshake(node_id.clone(),None).await?;
        connections.insert(node_id, conn);

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
        let node_id = node_addr.node_id.clone();
        let mut stream = self.tcp_socket.connect_node(node_addr).await?;
        stream.handshake(node_id, None).await?;
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
    /// Start listening for incoming network streams. 
    /// Returns a [`Listener`] that can be used to accept incoming streams.
    pub async fn listen(&mut self) -> Result<Listener> {
        let (sender, receiver) = mpsc::channel::<NetworkStream>(100);

        // Start the relay listener if a relay address is available
        if self.node_addr.relay_addr.is_some() {
            let relay_client = self.relay_client.clone();
            let cancellation_token = self.cancellation_token.clone();
            let relay_stream_sender = sender.clone();
            tokio::spawn(async move {
                match start_relay_listen_task(relay_client, cancellation_token, relay_stream_sender).await {
                    Ok(_) => {
                        tracing::info!("Relay listener stopped.");
                    }
                    Err(e) => {
                        tracing::error!("Error starting relay listener: {:?}", e);
                    }
                }
            });
        } else {
            tracing::warn!("Relay address not found. skipping relay listener.");
        }

        // Start the QUIC and TCP listeners for direct connections
        let quic_socket = self.quic_socket.clone();
        let tcp_socket = self.tcp_socket.clone();
        let cancellation_token = self.cancellation_token.clone();
        tokio::spawn(async move {
            match start_listen_task(quic_socket, tcp_socket, cancellation_token, sender).await {
                Ok(_) => {
                    tracing::info!("Endpoint listener stopped.");
                }
                Err(e) => {
                    tracing::error!("Error starting listener: {:?}", e);
                }
            }
        });
        
        Ok(Listener { receiver })
    }
    /// Remove stale or unused QUIC connections from the connection pool.
    pub async fn prune_quic_connections(&self) {
        let mut connections = self.quic_connections.lock().await;

        // Filter out connections that are no longer active or required.
        connections.retain(|node_id, connection| {
            let is_active = connection.is_active();
            if !is_active {
                tracing::info!("Removing inactive QUIC connection to node: {:?}", node_id);
            }
            is_active
        });

        tracing::info!(
            "Pruned QUIC connections. Remaining active connections: {}",
            connections.len()
        );
    }

    /// Gracefully shutdown the `Endpoint`.
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down Endpoint...");

        // Cancel any ongoing operations
        self.cancellation_token.cancel();
        tracing::info!("Cancellation token triggered.");

        // Clean up active QUIC connections
        let mut quic_connections = self.quic_connections.lock().await;
        for (node_id, mut connection) in quic_connections.drain() {
            tracing::info!("Closing QUIC connection to node: {:?}", node_id);
            if let Err(e) = connection.close().await {
                tracing::warn!("Error closing QUIC connection to {:?}: {:?}", node_id, e);
            }
        }
        tracing::info!("All QUIC connections closed.");

        tracing::info!("Endpoint shut down completed.");
        Ok(())
    }
}

async fn start_listen_task(quic_socket: QuicSocket, tcp_socket: TcpSocket, cancellation_token: CancellationToken, sender: mpsc::Sender<NetworkStream>) -> Result<()> {
    let (quic_conn_tx, mut quic_conn_rx) = mpsc::channel::<QuicConnection>(100);
    let mut quic_socket = quic_socket;
    let quic_cancel_token = cancellation_token.clone();
    tracing::info!("Starting QUIC listener...");
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
    let mut tcp_socket = tcp_socket;
    let tcp_cancel_token = cancellation_token.clone();
    tracing::info!("Starting TCP listener...");
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
            _ = cancellation_token.cancelled() => {
                tracing::info!("Endpoint listener cancelled.");
                break;
            }
            Some(mut conn) = quic_conn_rx.recv() => {
                tracing::info!("Accepted QUIC connection from: {}", conn.remote_address());
                let sender = sender.clone();
                tokio::spawn(async move {
                    match conn.accept_stream().await {
                        Ok(stream) => {
                            tracing::info!("Accepted QUIC stream from: {}", stream.remote_address());
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
                tracing::info!("Accepted TCP connection from: {}", conn.remote_address());
                let stream = NetworkStream::Tcp(conn);
                if let Err(e) = sender.send(stream).await {
                    tracing::error!("Error sending TCP connection: {:?}", e);
                }
            }
        }
    }
    Ok(())
}

async fn start_relay_listen_task(relay_client: RelayClient, cancellation_token: CancellationToken, sender: mpsc::Sender<NetworkStream>) -> Result<()> {
    let relay_addr = match &relay_client.node_addr.relay_addr {
        Some(addr) => addr.clone(),
        None => return Err(anyhow!("Relay address not found")),
    };
    let mut relay_client = relay_client;
    tracing::info!("Opening relay control stream...");
    let mut control_stream = relay_client.open_control_stream().await?;
    tracing::info!("Relay control stream opened.");

    loop {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                tracing::info!("Client actor loop cancelled, closing loop");
                break;
            }
            result = control_stream.receive_frame() => match result {
                Ok(frame) => {
                    match frame.frame_type {
                        FrameType::Connect => {
                            if let Some(payload) = frame.payload {
                                match payload {
                                    Payload::Handshake(handshake) => {
                                        match relay_client.connect(handshake.dst_node_id, relay_addr.clone()).await {
                                            Ok(stream) => {
                                                match sender.send(stream).await {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        tracing::error!("Error sending stream: {:?}", e);
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                tracing::error!("Error connecting to relay: {:?}", e);
                                            }
                                        }
                                    }
                                    _ => {
                                        tracing::warn!("Received unexpected payload type");
                                    }
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("Received unexpected frame type");
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving frame: {:?}", e);
                    // Check if the stream is closed
                    if let Some(e) = e.downcast_ref::<StreamError>() {
                        match e {
                            StreamError::Closed => {
                                tracing::info!("Relay control stream closed");
                                break;
                            }
                            _ => {
                                tracing::warn!("Error receiving frame: {:?}", e);
                            }
                        }
                    } else {
                        tracing::warn!("Error receiving frame: {:?}", e);
                    }
                }
            }           
        }
    }
    Ok(())
}
