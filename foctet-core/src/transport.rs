use std::{
    fmt,
    sync::atomic::{AtomicU32, Ordering},
};

use stackaddr::segment::protocol::TransportProtocol;
use anyhow::Result;

static NEXT_LISTENER_ID: AtomicU32 = AtomicU32::new(1);

/// The ID of a single listener.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(u32);

impl ListenerId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    /// Creates a new `ListenerId`.
    pub fn next() -> Self {
        ListenerId(NEXT_LISTENER_ID.fetch_add(1, Ordering::SeqCst))
    }
    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&mut self, n: u32) -> Self {
        let old = self.0;
        self.0 = self.0.checked_add(n).unwrap_or(0);
        ListenerId(old)
    }
}

impl fmt::Display for ListenerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents the kind of transport protocol, supported by foctet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransportKind {
    Quic,
    TlsTcp,
}

impl TransportKind {
    pub fn from_protocol(proto: TransportProtocol) -> Result<Self> {
        match proto {
            TransportProtocol::Udp(_) => Ok(TransportKind::Quic),
            TransportProtocol::Quic(_) => Ok(TransportKind::Quic),
            TransportProtocol::Tcp(_) => Ok(TransportKind::TlsTcp),
            TransportProtocol::TlsTcp(_) => Ok(TransportKind::TlsTcp),
            _ => Err(anyhow::anyhow!("Unsupported transport protocol: {:?}", proto)),
        }
    }
}
