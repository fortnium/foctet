use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub u32);

impl SessionId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The endpoint roles associated with a connection.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ConnectionSide {
    /// The socket comes from a connector.
    Connector,
    /// The socket comes from a listener.
    Listener,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Direction {
    /// The connection is incoming.
    Incoming,
    /// The connection is outgoing.
    Outgoing,
}
