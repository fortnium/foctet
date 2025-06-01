use std::collections::BTreeSet;

use crate::{id::NodeId, transport::TransportKind};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use stackaddr::{segment::protocol::TransportProtocol, StackAddr};

use anyhow::Result;
use base32::Alphabet;

/// Represents a node address. Network address information for a node.
/// Contains identifiers and addresses for direct connections and relay servers.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeAddr {
    /// The node ID of the node.
    pub node_id: NodeId,
    /// The addresses of the node for direct connections.
    pub addresses: BTreeSet<StackAddr>,
    /// The relay server information used to connect to this node.
    pub relay_addr: Option<RelayAddr>,
}

impl NodeAddr {
    /// Decodes a NodeAddr from a bytes. 
    pub fn from_bytes(bytes: Bytes) -> Result<Self> {
        let node_addr: Self = match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
            Ok((node_addr, _)) => node_addr,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to decode NodeAddr: {}", e));
            },
        };
        Ok(node_addr)
    }
    /// Encodes a NodeAddr to bytes.
    pub fn to_bytes(&self) -> Result<Bytes> {
        let serialized = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        Ok(Bytes::from(serialized))
    }
    /// Converts a RFC4648 base32 string into a NodeAddr.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        let node_addr: Self = match bincode::serde::decode_from_slice(&decoded, bincode::config::standard()) {
            Ok((node_addr, _)) => node_addr,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to decode base32 string: {}", e));
            },
        };
        Ok(node_addr)
    }
    /// Converts the NodeAddr to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        Ok(base32::encode(Alphabet::Rfc4648 { padding: false }, &serialized))
    }

    pub fn get_direct_addrs(&self, transport_id: &TransportKind, allow_loopback: bool) -> Vec<StackAddr> {
        let mut addrs = Vec::new();
        for addr in &self.addresses {
            match addr.ip() {
                Some(ip) => {
                    if !allow_loopback && ip.is_loopback() {
                        continue;
                    }
                }
                None => {
                    continue;
                }
            }
            match transport_id {
                TransportKind::Quic => {
                    match addr.transport() {
                        Some(TransportProtocol::Quic(_)) | Some(TransportProtocol::Udp(_)) => {
                            addrs.push(addr.clone());
                        }
                        _ => {}
                    }
                }
                TransportKind::TlsOverTcp => {
                    match addr.transport() {
                        Some(TransportProtocol::Tcp(_)) | Some(TransportProtocol::TlsOverTcp(_)) => {
                            addrs.push(addr.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
        addrs
    }
}

/// Represents a node address.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RelayAddr {
    /// The node ID of the node.
    pub node_id: NodeId,
    /// The transport addresses of the node for direct connections.
    pub addresses: BTreeSet<StackAddr>,
}

impl RelayAddr {
    /// Converts a RFC4648 base32 string into a RelayAddr.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        let relay_addr: Self = match bincode::serde::decode_from_slice(&decoded, bincode::config::standard()) {
            Ok((relay_addr, _)) => relay_addr,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to decode base32 string: {}", e));
            },
        };
        Ok(relay_addr)
    }
    /// Converts the RelayAddr to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        Ok(base32::encode(Alphabet::Rfc4648 { padding: false }, &serialized))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodePair {
    pub src: NodeId,
    pub dst: NodeId,
}

impl NodePair {
    pub fn new(src: NodeId, dst: NodeId) -> Self {
        Self { src, dst }
    }
}
