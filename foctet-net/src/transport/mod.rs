pub mod connection;
pub mod stream;

pub mod quic;
pub mod tcp;

use std::future::Future;

use anyhow::Result;
use connection::{Connection, ConnectionEvent};
use foctet_core::transport::{ListenerId, TransportKind};
use quic::transport::QuicTransport;
use stackaddr::StackAddr;
use tcp::transport::TcpTransport;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// A listener for incoming network connection.
pub struct ConnectionListener {
    listener_id: ListenerId,
    receiver: mpsc::Receiver<ConnectionEvent>,
    cancel_token: CancellationToken,
}

impl ConnectionListener {
    pub fn new(
        listener_id: ListenerId,
        receiver: mpsc::Receiver<ConnectionEvent>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            listener_id,
            receiver,
            cancel_token,
        }
    }
    /// Accept a new (next) stream
    pub async fn accept(&mut self) -> Option<ConnectionEvent> {
        self.receiver.recv().await
    }
    /// Get the listener ID.
    pub fn listener_id(&self) -> ListenerId {
        self.listener_id
    }
    /// Cancel the listener.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
}

// A sender for sending connection events.
pub struct ConnectionSender {
    sender: mpsc::Sender<ConnectionEvent>,
}

impl ConnectionSender {
    pub fn new(sender: mpsc::Sender<ConnectionEvent>) -> Self {
        Self { sender }
    }
    /// Send a connection event.
    pub async fn send(&mut self, event: ConnectionEvent) -> Result<()> {
        self.sender.send(event).await?;
        Ok(())
    }
}

pub trait TransportHandle: Send + Sync {
    fn listen_on(
        &mut self,
        id: ListenerId,
        addr: StackAddr,
    ) -> impl Future<Output = Result<ConnectionListener>> + Send;
    fn connect(&mut self, addr: StackAddr) -> impl Future<Output = Result<Connection>> + Send;
}

pub enum Transport {
    Quic(QuicTransport),
    Tcp(TcpTransport),
}

impl Transport {
    pub fn transport_kind(&self) -> TransportKind {
        match self {
            Self::Quic(_) => TransportKind::Quic,
            Self::Tcp(_) => TransportKind::TlsOverTcp,
        }
    }
    pub async fn listen_on(
        &mut self,
        id: ListenerId,
        addr: StackAddr,
    ) -> Result<ConnectionListener> {
        match self {
            Self::Quic(transport) => transport.listen_on(id, addr).await,
            Self::Tcp(transport) => transport.listen_on(id, addr).await,
        }
    }

    pub async fn connect(&mut self, addr: StackAddr) -> Result<Connection> {
        match self {
            Self::Quic(transport) => transport.connect(addr).await,
            Self::Tcp(transport) => transport.connect(addr).await,
        }
    }
}
