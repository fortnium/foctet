use foctet_core::node::NodeId;
use crate::{config::SocketConfig, connection::{quic::QuicSocket, tcp::TcpSocket}};
use anyhow::Result;

/// The type of socket and transport protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketType {
    /// QUIC socket (default)
    Quic,
    /// TCP socket
    Tcp,
    /// Both QUIC and TCP
    Both,
}

impl Default for SocketType {
    fn default() -> Self {
        SocketType::Both
    }
}

pub struct Socket {
    pub socket_type: SocketType,
    pub quic_socket: QuicSocket,
    pub tcp_socket: TcpSocket,
}

impl Socket {
    pub fn new(node_id: NodeId, config: SocketConfig) -> Result<Self> {
        let socket_type = config.socket_type.clone();
        let quic_socket = QuicSocket::new(node_id.clone(), config.clone())?;
        let tcp_socket = TcpSocket::new(node_id, config)?;
        Ok(Self {
            socket_type,
            quic_socket,
            tcp_socket,
        })
    }
}
