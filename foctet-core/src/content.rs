use serde::{Serialize, Deserialize};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use anyhow::Result;
use crate::{key::{self, UUID_V4_BYTES_LEN}, node::NodeAddr};

/// The content ID for a payload
/// 128-bit UUID (Universally Unique Identifier) v4 is used.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ContentId([u8; UUID_V4_BYTES_LEN]);

impl ContentId {
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
    /// Return the hexadecimal representation of the content ID.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
    /// Check if the connection ID is empty.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
    /// Converts a base64 string into a ContentId.
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let decoded = URL_SAFE.decode(encoded)?;
        let node_addr: Self = bincode::deserialize(&decoded)?;
        Ok(node_addr)
    }
    /// Converts the ContentId to a single base64 string.
    pub fn to_base64(&self) -> Result<String> {
        let serialized = bincode::serialize(self)?;
        Ok(URL_SAFE.encode(&serialized))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransferTicket {
    pub node_addr: NodeAddr,
    pub content_id: ContentId,
}

impl TransferTicket {
    /// Create a new transfer ticket with the given node address and content ID.
    pub fn new(node_addr: NodeAddr, content_id: ContentId) -> Self {
        Self { node_addr, content_id }
    }
    /// Converts a base64 string into a TransferTicket.
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let decoded = URL_SAFE.decode(encoded)?;
        let node_addr: Self = bincode::deserialize(&decoded)?;
        Ok(node_addr)
    }
    /// Converts the TransferTicket to a single base64 string.
    pub fn to_base64(&self) -> Result<String> {
        let serialized = bincode::serialize(self)?;
        Ok(URL_SAFE.encode(&serialized))
    }
}
