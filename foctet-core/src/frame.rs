use serde::{Serialize, Deserialize};
use crate::{hash::Blake3Hash, key::{self, UUID_V4_BYTES_LEN}, node::{ConnectionId, NodeId}, time::UnixTimestamp};

/// The frame structure that is sent between the peers
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    /// The header of the frame containing metadata for routing and identification
    pub header: FrameHeader,
    /// The payload of the frame. This can be any data that needs to be sent between the peers.
    /// For file transfers, this will be the file binary data.
    /// For messages, this will be the message string. encoded in UTF-8.
    pub payload: Option<Payload>,
}

impl Frame {
    /// Create a new frame with the given header and payload.
    pub fn new(header: FrameHeader, payload: Option<Payload>) -> Self {
        Self { header, payload }
    }
    pub fn empty() -> Self {
        Self {
            header: FrameHeader {
                frame_type: FrameType::Message,
                node_id: NodeId::zero(),
                connection_id: None,
                content_id: None,
            },
            payload: None,
        }
    }
    /// Convert the frame to a byte array
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
    /// Convert a byte array to a frame
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

/// The frame header containing metadata for routing and identification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// The type of the frame
    pub frame_type: FrameType,
    /// The node ID of the sender (source)
    pub node_id: NodeId,
    /// The connection ID between the sender and the receiver
    pub connection_id: Option<ConnectionId>,
    /// The content ID of the payload
    pub content_id: Option<ContentId>,
}

/// The different types of frames in the protocol
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum FrameType {
    Connect,
    Connected,
    Disconnect,
    Disconnected,
    Message,
    DataTransfer,
    FileTransfer,
}

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
}

/// Represents the payload of the message
///
/// This can be a file chunk or a text message
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Payload {
    #[serde(with = "serde_bytes")]
    DataChunk(Vec<u8>),
    #[serde(with = "serde_bytes")]
    FileChunk(Vec<u8>),
    Text(String),
    FileMetadata(FileMetadata),
    Metadata(Metadata),
}

/// Represents the metadata of the content
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub name: String,
    pub size: usize,
    pub hash: String,
    pub created: u64,
    pub modified: u64,
}

/// Represents the metadata of the file or compressed directory
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub name: String,
    pub size: usize,
    pub hash: Blake3Hash,
    pub is_directory: bool,
    /// File created timestamp in Unix time
    pub created: UnixTimestamp,
    /// File modified timestamp in Unix time
    pub modified: UnixTimestamp,
    /// File accessed timestamp in Unix time
    pub accessed: UnixTimestamp,
}
