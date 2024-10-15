use serde::{Serialize, Deserialize};
use crate::{key::{self, UUID_V4_BYTES_LEN}, node::{ConnectionId, NodeId}};

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

/// The frame header containing metadata for routing and identification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// The type of the frame
    pub frame_type: FrameType,
    /// The node ID of the sender (source)
    pub node_id: NodeId,
    /// The connection ID between the sender and the receiver
    pub connection_id: Option<ConnectionId>,
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
}

/// The type of the payload
/// This can be a file chunk or a text message
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum PayloadType {
    FileChunk,
    TextMessage,
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
    /// Check if the connection ID is empty.
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
}

/// Represents the payload of the message
///
/// This can be a file chunk or a text message
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Payload {
    pub payload_type: PayloadType,
    pub content_id: ContentId,
    pub data: Vec<u8>,
}

/// Represents the metadata of the content
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

/// Represents the metadata of the file or compressed directory
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
    pub hash: String,
    pub is_directory: bool,
}
