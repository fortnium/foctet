use crate::{addr::NamedSocketAddr, key::{self, NodePublicKey, UUID_V4_BYTES_LEN}};
use anyhow::Result;
use base32::Alphabet;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, net::SocketAddr};

/// The identifier for a node in the foctet network.
/// This is the ED25519 public key of the node, with length 32 bytes.
pub type NodeId = NodePublicKey;

/// Represents a node address. Network address information for a node.
/// Contains identifiers and addresses for direct connections and relay servers.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeAddr {
    /// The node ID of the node.
    pub node_id: NodeId,
    /// The hostname of the node.
    pub server_name: Option<String>,
    /// The QUIC/TCP socket address of the node for direct connections.
    pub socket_addresses: BTreeSet<SocketAddr>,
    /// The relay server information used to connect to this node.
    pub relay_addr: Option<RelayAddr>,
}

impl NodeAddr {
    /// Create a new node address with the given node ID and socket address.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            server_name: None,
            socket_addresses: BTreeSet::new(),
            relay_addr: None,
        }
    }
    /// Create a new node from another node address.
    pub fn from_node(node_addr: &NodeAddr) -> Self {
        Self {
            node_id: node_addr.node_id.clone(),
            server_name: node_addr.server_name.clone(),
            socket_addresses: node_addr.socket_addresses.clone(),
            relay_addr: node_addr.relay_addr.clone(),
        }
    }
    pub fn with_named_socket_addr(mut self, named_socket_addr: NamedSocketAddr) -> Self {
        if let Some(iter) = named_socket_addr.to_socket_addrs() {
            for socket_addr in iter {
                self.socket_addresses.insert(socket_addr);
            }
        }
        self.server_name = Some(named_socket_addr.host);
        self
    }
    pub fn with_server_name(mut self, server_name: String) -> Self {
        self.server_name = Some(server_name);
        self
    }
    pub fn with_socket_addr(mut self, socket_addr: SocketAddr) -> Self {
        self.socket_addresses.insert(socket_addr);
        self
    }
    /// Add a socket address to the node address.
    pub fn add_socket_addr(&mut self, socket_addr: SocketAddr) {
        self.socket_addresses.insert(socket_addr);
    }
    pub fn with_socket_addresses(mut self, socket_addresses: BTreeSet<SocketAddr>) -> Self {
        self.socket_addresses = socket_addresses;
        self
    }
    /// Set relay address.
    pub fn with_relay(mut self, relay_addr: RelayAddr) -> Self {
        self.relay_addr = Some(relay_addr);
        self
    }
    /// Create a unspecifed node address with zero node ID and unspecified socket address.
    pub fn unspecified() -> Self {
        Self {
            node_id: NodePublicKey::zero(),
            server_name: None,
            socket_addresses: BTreeSet::new(),
            relay_addr: None,
        }
    }
    /// Check if the node address is unspecified.
    pub fn is_unspecified(&self) -> bool {
        self.node_id.is_zero()
    }
    /// Converts a RFC4648 base32 string into a ContentId.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        let node_addr: Self = bincode::deserialize(&decoded)?;
        Ok(node_addr)
    }
    /// Converts the ContentId to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serialize(self)?;
        Ok(base32::encode(Alphabet::Rfc4648 { padding: false }, &serialized))
    }
    /// Get socket address.
    /// Returns the first socket address in the set.
    pub fn get_socket_addr(&self) -> Option<SocketAddr> {
        self.socket_addresses.iter().next().cloned()
    }
    /// Get the server name
    pub fn get_server_name(&self) -> String {
        match &self.server_name {
            Some(name) => name.clone(),
            None => {
                if self.is_unspecified() || self.socket_addresses.is_empty() {
                    return String::new();
                } else {
                    // if socket_addresses contains std::net::Ipv4Addr::LOCALHOST std::net::Ipv6Addr::LOCALHOST
                    // return "localhost"
                    for socket_addr in &self.socket_addresses {
                        if socket_addr.ip().is_loopback() {
                            return "localhost".to_string();
                        }
                    }
                    // return the first socket address
                    match self.socket_addresses.iter().next() {
                        Some(socket_addr) => socket_addr.ip().to_string(),
                        None => String::new(),
                    }
                }
            },
        }
    }
}

/// The connection ID for a connection.
/// 128-bit UUID (Universally Unique Identifier) v4 is used.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ConnectionId([u8; UUID_V4_BYTES_LEN]);

impl ConnectionId {
    /// Create a new connection ID with the given string.
    pub fn new() -> Self {
        Self(key::generate_uuid_v4_bytes())
    }
    /// Create an zero connection ID.
    pub fn zero() -> Self {
        Self([0; UUID_V4_BYTES_LEN])
    }
    /// Get the connection ID as a string slice.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
    /// Check if the connection ID is empty.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
    /// Converts a RFC4648 base32 string into a ContentId.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        let node_addr: Self = bincode::deserialize(&decoded)?;
        Ok(node_addr)
    }
    /// Converts the ContentId to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serialize(self)?;
        Ok(base32::encode(Alphabet::Rfc4648 { padding: false }, &serialized))
    }
}

/// Represents a relay server address.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RelayAddr {
    /// The node ID of the relay server.
    pub relay_node_id: NodeId,
    /// The hostname of the relay server.
    pub relay_server_name: Option<String>,
    /// The QUIC/TCP socket address of the relay server.
    pub socket_addresses: BTreeSet<SocketAddr>,
    /// Connection ID for the relay server.
    pub relay_connection_id: Option<ConnectionId>,
}

impl RelayAddr {
    /// Create a new relay address with the given relay node ID and socket address.
    pub fn new(relay_node_id: NodeId) -> Self {
        Self {
            relay_node_id,
            relay_server_name: None,
            socket_addresses: BTreeSet::new(),
            relay_connection_id: None,
        }
    }
    pub fn unspecified() -> Self {
        Self {
            relay_node_id: NodePublicKey::zero(),
            relay_server_name: None,
            socket_addresses: BTreeSet::new(),
            relay_connection_id: None,
        }
    }
    pub fn with_socket_addr(mut self, socket_addr: SocketAddr) -> Self {
        self.socket_addresses.insert(socket_addr);
        self
    }
    pub fn with_socket_addresses(mut self, socket_addresses: BTreeSet<SocketAddr>) -> Self {
        self.socket_addresses = socket_addresses;
        self
    }
    /// Add a socket address to the node address.
    pub fn add_socket_addr(&mut self, socket_addr: SocketAddr) {
        self.socket_addresses.insert(socket_addr);
    }
    pub fn with_named_socket_addr(mut self, named_socket_addr: NamedSocketAddr) -> Self {
        if let Some(iter) = named_socket_addr.to_socket_addrs() {
            for socket_addr in iter {
                self.socket_addresses.insert(socket_addr);
            }
        }
        self.relay_server_name = Some(named_socket_addr.host);
        self
    }
    pub fn with_server_name(mut self, server_name: String) -> Self {
        self.relay_server_name = Some(server_name);
        self
    }
    /// Set connection ID.
    pub fn with_connection_id(mut self, connection_id: ConnectionId) -> Self {
        self.relay_connection_id = Some(connection_id);
        self
    }
    /// Check if the node address is unspecified.
    pub fn is_unspecified(&self) -> bool {
        self.relay_node_id.is_zero()
    }
    /// Get the server name
    pub fn get_server_name(&self) -> String {
        match &self.relay_server_name {
            Some(name) => name.clone(),
            None => {
                if self.is_unspecified() || self.socket_addresses.is_empty() {
                    return String::new();
                } else {
                    // if socket_addresses contains std::net::Ipv4Addr::LOCALHOST std::net::Ipv6Addr::LOCALHOST
                    // return "localhost"
                    for socket_addr in &self.socket_addresses {
                        if socket_addr.ip().is_loopback() {
                            return "localhost".to_string();
                        }
                    }
                    // return the first socket address
                    match self.socket_addresses.iter().next() {
                        Some(socket_addr) => socket_addr.ip().to_string(),
                        None => String::new(),
                    }
                }
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct NodeConnection {
    /// The node ID of the node.
    pub node_id: NodeId,
    /// Connection ID for the node.
    pub connection_id: ConnectionId,
}

impl NodeConnection {
    /// Create a new node connection with the given node ID and connection ID.
    pub fn new(node_id: NodeId, connection_id: ConnectionId) -> Self {
        Self {
            node_id,
            connection_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use super::*;

    #[test]
    fn test_node_addr() {
        let node_id = NodeId::generate();
        let node_addr = NodeAddr::new(node_id)
            .with_socket_addr(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 4432)));
        println!("NodeAddr: {:?}", node_addr);
        let id = node_addr.to_base32().unwrap();
        println!("NodeAddr ID: {}", id);
        let node_addr2 = NodeAddr::from_base32(&id).unwrap();
        println!("NodeAddr2: {:?}", node_addr2);
        assert_eq!(node_addr, node_addr2);
    }
}
