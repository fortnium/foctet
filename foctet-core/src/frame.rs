use std::fmt;

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
    /// Create a new frame with default empty header and no payload.
    pub fn empty() -> Self {
        Self {
            header: FrameHeader {
                frame_type: FrameType::Message,
                node_id: NodeId::zero(),
                stream_id: StreamId(0),
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
    /// Create TransferStart frame
    pub fn transfer_start(node_id: NodeId, stream_id: StreamId, connection_id: Option<ConnectionId>, content_id: Option<ContentId>) -> Self {
        Self {
            header: FrameHeader {
                frame_type: FrameType::TransferStart,
                node_id,
                stream_id,
                connection_id,
                content_id,
            },
            payload: None,
        }
    }
    /// Create EndOfTransfer frame
    pub fn end_of_transfer(node_id: NodeId, stream_id: StreamId, connection_id: Option<ConnectionId>, content_id: Option<ContentId>) -> Self {
        Self {
            header: FrameHeader {
                frame_type: FrameType::EndOfTransfer,
                node_id,
                stream_id,
                connection_id,
                content_id,
            },
            payload: None,
        }
    }
    /// Get length of payload
    pub fn payload_len(&self) -> usize {
        match &self.payload {
            Some(payload) => payload.len(),
            None => 0,
        }
    }
}

/// The frame header containing metadata for routing and identification
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// The type of the frame
    pub frame_type: FrameType,
    /// The node ID of the sender (source)
    pub node_id: NodeId,
    /// The stream ID of the frame
    pub stream_id: StreamId,
    /// The connection ID between the sender and the receiver
    pub connection_id: Option<ConnectionId>,
    /// The content ID of the payload
    pub content_id: Option<ContentId>,
}

impl FrameHeader {
    /// Starts building a new `FrameHeader` using `FrameHeaderBuilder`.
    pub fn builder() -> FrameHeaderBuilder {
        FrameHeaderBuilder::new()
    }
}

pub struct FrameHeaderBuilder {
    frame_type: FrameType,
    node_id: NodeId,
    stream_id: StreamId,
    connection_id: Option<ConnectionId>,
    content_id: Option<ContentId>,
}

impl FrameHeaderBuilder {
    /// Creates a new `FrameHeaderBuilder` with the required fields.
    pub fn new() -> Self {
        Self {
            frame_type: FrameType::Message,
            node_id: NodeId::zero(),
            stream_id: StreamId(0),
            connection_id: None,
            content_id: None,
        }
    }
    /// Sets the frame type.
    pub fn with_frame_type(mut self, frame_type: FrameType) -> Self {
        self.frame_type = frame_type;
        self
    }
    /// Sets the node ID.
    pub fn with_node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = node_id;
        self
    }
    /// Sets the stream ID.
    pub fn with_stream_id(mut self, stream_id: StreamId) -> Self {
        self.stream_id = stream_id;
        self
    }
    /// Sets the connection ID.
    pub fn with_connection_id(mut self, connection_id: ConnectionId) -> Self {
        self.connection_id = Some(connection_id);
        self
    }

    /// Sets the content ID.
    pub fn with_content_id(mut self, content_id: ContentId) -> Self {
        self.content_id = Some(content_id);
        self
    }

    /// Builds the `FrameHeader`.
    pub fn build(self) -> FrameHeader {
        FrameHeader {
            frame_type: self.frame_type,
            node_id: self.node_id,
            stream_id: self.stream_id,
            connection_id: self.connection_id,
            content_id: self.content_id,
        }
    }
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
    TransferStart,
    EndOfTransfer,
}

/// Identifier for a stream within a particular connection
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StreamId(pub u64);

impl StreamId {
    /// Create a new stream ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    /// Get the stream ID as a string
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
    /// Increment the stream ID by one
    pub fn increment(&mut self) {
        self.0 += 1;
    }
    /// Decrement the stream ID by one
    /// If the stream ID is zero, it will remain zero.
    pub fn decrement(&mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamId({})", self.0)
    }
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

impl Payload {
    /// Get the size of the payload
    pub fn len(&self) -> usize {
        match self {
            Self::DataChunk(data) => data.len(),
            Self::FileChunk(data) => data.len(),
            Self::Text(text) => text.len(),
            Self::FileMetadata(metadata) => {
                // Serialize the metadata to bytes and get the size
                bincode::serialize(metadata).unwrap_or_default().len()
            },
            Self::Metadata(metadata) => {
                // Serialize the metadata to bytes and get the size
                bincode::serialize(metadata).unwrap_or_default().len()
            },
        }
    }
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
