use anyhow::Result;
use base32::Alphabet;
use bincode::{Decode, Encode};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::key::PublicKey;
pub const UUID_V4_BYTES_LEN: usize = 16;

/// The identifier for a node in the foctet network.
/// This is the ED25519 public key of the node, with length 32 bytes.
pub type NodeId = PublicKey;

/// The unique identifier for a connection or content in the foctet network.
/// 128-bit UUID (Universally Unique Identifier) v4 is used.
#[derive(Serialize, Deserialize, Encode, Decode, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UuidV4([u8; UUID_V4_BYTES_LEN]);

impl UuidV4 {
    /// Create a new random UUID.
    pub fn new() -> Self {
        let unique_id: Uuid = Uuid::new_v4();
        let mut id = [0u8; UUID_V4_BYTES_LEN];
        id.copy_from_slice(unique_id.as_bytes());
        Self(id)
    }
    /// Create an zero connection ID.
    pub fn zero() -> Self {
        Self([0; UUID_V4_BYTES_LEN])
    }
    /// Get the connection ID as a string slice.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
    pub fn from_bytes(bytes: Bytes) -> Result<Self> {
        if bytes.len() != UUID_V4_BYTES_LEN {
            return Err(anyhow::anyhow!(
                "Invalid length for UuidV4: expected {}, got {}",
                UUID_V4_BYTES_LEN,
                bytes.len()
            ));
        }
        let mut id = [0u8; UUID_V4_BYTES_LEN];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
    pub fn to_bytes(&self) -> Bytes {
        Bytes::copy_from_slice(&self.0)
    }
    /// Converts a hex encoded string into a UuidV4.
    pub fn from_hex(encoded: &str) -> Result<Self> {
        let bytes = hex::decode(encoded)?;
        let mut id = [0u8; UUID_V4_BYTES_LEN];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
    /// Return the hexadecimal representation of the content ID.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
    /// Check if the connection ID is empty.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
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
