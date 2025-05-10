use anyhow::Result;
use base32::Alphabet;
use serde::{Deserialize, Serialize};

use crate::{addr::node::NodeAddr, id::UuidV4};

/// The content ID for a payload
/// 128-bit UUID (Universally Unique Identifier) v4 is used.
pub type ContentId = UuidV4;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransferTicket {
    pub node_addr: NodeAddr,
    pub content_id: ContentId,
}

impl TransferTicket {
    /// Create a new transfer ticket with the given node address and content ID.
    pub fn new(node_addr: NodeAddr, content_id: UuidV4) -> Self {
        Self {
            node_addr,
            content_id,
        }
    }
    /// Converts a RFC4648 base32 string into a ContentId.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        match bincode::serde::decode_from_slice(&decoded, bincode::config::standard()) {
            Ok((node_addr, _)) => Ok(node_addr),
            Err(e) => Err(anyhow::anyhow!("Failed to decode base32 string: {}", e)),
        }
    }
    /// Converts the ContentId to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        Ok(base32::encode(
            Alphabet::Rfc4648 { padding: false },
            &serialized,
        ))
    }
}
