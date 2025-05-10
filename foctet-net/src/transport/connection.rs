use std::future::Future;

use anyhow::Result;
use foctet_core::{connection::Direction, id::NodeId};
use stackaddr::StackAddr;

use super::{quic::connection::QuicConnection, stream::Stream, tcp::connection::TcpConnection};

pub trait ConnectionHandle: Send + Sync {
    fn direction(&self) -> Direction;
    fn local_addr(&self) -> StackAddr;
    fn remote_addr(&self) -> StackAddr;
    fn local_node_id(&self) -> NodeId;
    fn remote_node_id(&self) -> NodeId;
    fn open_stream(&mut self) -> impl Future<Output = Result<Stream>> + Send;
    fn accept_stream(&mut self) -> impl Future<Output = Result<Stream>> + Send;
    fn close(&mut self) -> impl Future<Output = Result<()>> + Send;
}

/// Represents an event that occurs during a connection attempt.
pub enum ConnectionEvent {
    /// The connection was established.
    Connected(Connection),
    /// The connection was accepted.
    Accepted(Connection),
}

pub enum Connection {
    Quic(QuicConnection),
    Tcp(TcpConnection),
}

impl Connection {
    pub fn direction(&self) -> Direction {
        match self {
            Connection::Quic(conn) => conn.direction(),
            Connection::Tcp(conn) => conn.direction(),
        }
    }
    pub fn local_addr(&self) -> StackAddr {
        match self {
            Connection::Quic(conn) => conn.local_addr(),
            Connection::Tcp(conn) => conn.local_addr(),
        }
    }
    pub fn remote_addr(&self) -> StackAddr {
        match self {
            Connection::Quic(conn) => conn.remote_addr(),
            Connection::Tcp(conn) => conn.remote_addr(),
        }
    }
    pub fn local_node_id(&self) -> NodeId {
        match self {
            Connection::Quic(conn) => conn.local_node_id(),
            Connection::Tcp(conn) => conn.local_node_id(),
        }
    }
    pub fn remote_node_id(&self) -> NodeId {
        match self {
            Connection::Quic(conn) => conn.remote_node_id(),
            Connection::Tcp(conn) => conn.remote_node_id(),
        }
    }
    pub async fn open_stream(&mut self) -> Result<Stream> {
        match self {
            Connection::Quic(conn) => conn.open_stream().await,
            Connection::Tcp(conn) => conn.open_stream().await,
        }
    }
    pub async fn accept_stream(&mut self) -> Result<Stream> {
        match self {
            Connection::Quic(conn) => conn.accept_stream().await,
            Connection::Tcp(conn) => conn.accept_stream().await,
        }
    }
    pub async fn close(&mut self) -> Result<()> {
        match self {
            Connection::Quic(conn) => conn.close().await,
            Connection::Tcp(conn) => conn.close().await,
        }
    }
}
