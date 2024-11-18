use std::{fmt, path::PathBuf};

use crate::{
    content::ContentId,
    hash::Blake3Hash,
    node::{ConnectionId, NodeId},
    time::UnixTimestamp,
};
use serde::{Deserialize, Serialize};

/// The frame structure that is sent between the peers
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    /// Indicates if this is the final frame in a sequence of frames for a particular operation.
    pub fin: bool,
    /// The type of the frame, used to distinguish different operations such as data transfer, connect, or disconnect.
    pub frame_type: FrameType,
    /// A unique identifier for the operation, allowing tracking of individual send/receive operations.
    /// This ID can be used for error handling, retransmissions, and associating responses with requests.
    pub operation_id: OperationId,
    /// Flags providing additional information about the frame.
    /// Each bit in this field can represent a specific flag, such as indicating fragmentation, priority, or compression.
    pub flags: u8,
    /// The payload of the frame. This can be any data that needs to be sent between the peers.
    /// For file transfers, this will be the file binary data.
    /// For messages, this will be the message string. encoded in UTF-8.
    pub payload: Option<Payload>,
}

impl Frame {
    /// Create a new frame with default empty header and no payload.
    pub fn empty() -> Self {
        Self {
            fin: false,
            frame_type: FrameType::Text,
            operation_id: OperationId(0),
            flags: 0,
            payload: None,
        }
    }
    /// Starts building a new `Frame` using `FrameBuilder`.
    pub fn builder() -> FrameBuilder {
        FrameBuilder::new()
    }
    /// Convert the frame to a byte array
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
    /// Convert a byte array to a frame
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
    /// Get legnth of the frame
    pub fn len(&self) -> usize {
        bincode::serialize(self).unwrap_or_default().len()
    }
    /// Get length of payload
    pub fn payload_len(&self) -> usize {
        match &self.payload {
            Some(payload) => payload.len(),
            None => 0,
        }
    }
}

pub struct FrameBuilder {
    /// Indicates if this is the final frame in a sequence of frames for a particular operation.
    fin: bool,
    /// The type of the frame, used to distinguish different operations such as data transfer, connect, or disconnect.
    frame_type: FrameType,
    /// A unique identifier for the operation, allowing tracking of individual send/receive operations.
    /// This ID can be used for error handling, retransmissions, and associating responses with requests.
    operation_id: OperationId,
    /// Flags providing additional information about the frame.
    /// Each bit in this field can represent a specific flag, such as indicating fragmentation, priority, or compression.
    flags: u8,
    /// The payload of the frame. This can be any data that needs to be sent between the peers.
    /// For file transfers, this will be the file binary data.
    /// For messages, this will be the message string. encoded in UTF-8.
    payload: Option<Payload>,
}

impl FrameBuilder {
    /// Creates a new `FrameHeaderBuilder` with the required fields.
    pub fn new() -> Self {
        Self {
            fin: false,
            frame_type: FrameType::Text,
            operation_id: OperationId(0),
            flags: 0,
            payload: None,
        }
    }
    /// Sets the fin flag.
    pub fn with_fin(mut self, fin: bool) -> Self {
        self.fin = fin;
        self
    }
    /// Sets the frame type.
    pub fn with_frame_type(mut self, frame_type: FrameType) -> Self {
        self.frame_type = frame_type;
        self
    }
    /// Sets the operation ID.
    pub fn with_operation_id(mut self, operation_id: OperationId) -> Self {
        self.operation_id = operation_id;
        self
    }
    /// Sets the flags.
    pub fn with_flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }
    /// Sets the payload.
    pub fn with_payload(mut self, payload: Payload) -> Self {
        self.payload = Some(payload);
        self
    }
    /// Builds the `Frame`.
    pub fn build(self) -> Frame {
        Frame {
            fin: self.fin,
            frame_type: self.frame_type,
            operation_id: self.operation_id,
            flags: self.flags,
            payload: self.payload,
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
    Text,
    DataTransfer,
    FileTransfer,
    TransferStart,
    EndOfTransfer,
    ContentRequest,
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

/// Identifier for a operation within a particular stream
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OperationId(pub u64);

impl OperationId {
    /// Create a new operation ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    /// Get the operation ID as a string
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
    /// Increment the operation ID by one
    pub fn increment(&mut self) {
        self.0 += 1;
    }
    /// Decrement the operation ID by one
    /// If the operation ID is zero, it will remain zero.
    pub fn decrement(&mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OperationId({})", self.0)
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
    ConnectionInfo(ConnectionInfo),
    ContentId(ContentId),
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
            }
            Self::Metadata(metadata) => {
                // Serialize the metadata to bytes and get the size
                bincode::serialize(metadata).unwrap_or_default().len()
            }
            Self::ConnectionInfo(info) => {
                // Serialize the connection info to bytes and get the size
                bincode::serialize(info).unwrap_or_default().len()
            }
            Self::ContentId(id) => {
                // Serialize the content ID to bytes and get the size
                bincode::serialize(id).unwrap_or_default().len()
            }
        }
    }
    /// Create a new data chunk payload
    pub fn data_chunk(data: Vec<u8>) -> Self {
        Self::DataChunk(data)
    }
    /// Create a new file chunk payload
    pub fn file_chunk(data: Vec<u8>) -> Self {
        Self::FileChunk(data)
    }
    /// Create a new text payload
    pub fn text(text: String) -> Self {
        Self::Text(text)
    }
    /// Create a new file metadata payload
    pub fn file_metadata(metadata: FileMetadata) -> Self {
        Self::FileMetadata(metadata)
    }
    /// Create a new metadata payload
    pub fn metadata(metadata: Metadata) -> Self {
        Self::Metadata(metadata)
    }
    /// Create a new connection info payload
    pub fn connection_info(info: ConnectionInfo) -> Self {
        Self::ConnectionInfo(info)
    }
    /// Create a new content ID payload
    pub fn content_id(id: ContentId) -> Self {
        Self::ContentId(id)
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

/// Represents the metadata of the file or compressed directory
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct LocalFileMetadata {
    pub name: String,
    pub path: PathBuf,
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

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ConnectionInfo {
    pub node_id: NodeId,
    pub connection_id: Option<ConnectionId>,
}
