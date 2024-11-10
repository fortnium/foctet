pub mod actor;

use crate::{
    config::SocketConfig,
    connection::{
        quic::{QuicConnection, QuicSocket},
        tcp::{TcpSocket, TlsTcpStream},
        FoctetStream,
    },
};
use anyhow::Result;
use foctet_core::{
    error::ConnectionError,
    node::{NodeAddr, NodeId},
};
use std::net::SocketAddr;
use tokio::sync::mpsc;

/// The type of socket and transport protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketType {
    /// QUIC socket
    Quic,
    /// TCP socket
    Tcp,
    /// Both QUIC and TCP (default)
    Both,
}

impl Default for SocketType {
    fn default() -> Self {
        SocketType::Both
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
