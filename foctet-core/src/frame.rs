use bitflags::bitflags;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::stream::{OperationId, StreamId};
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Fixed size of the FrameHeader in bytes.
/// Consists of:
/// - stream_id: u32 (4 bytes)
/// - operation_id: u32 (4 bytes)
/// - flags: u8 (1 byte)
/// - frame_type: u8 (1 byte)
/// - payload_len: u32 (4 bytes)
pub const FRAME_HEADER_LEN: usize = 14;

bitflags! {
    /// The flags for frames, including stream control flags. 
    /// They can be combined using bitwise operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FrameFlags: u8 {
        /// No flags are set.
        const EMPTY = 0b0000_0000;
        /// Indicates that this frame is a request.
        /// For example, opening or closing a stream.
        const REQUEST = 0b0000_0001;
        /// Indicates that this frame is a response to a request.
        const RESPONSE = 0b0000_0010;
        /// Indicates that this frame is part of a fragmented message.
        /// Used when a payload is split across multiple frames.
        const FRAGMENTED = 0b0000_0100;
        /// Indicates that this frame is the final frame of an operation (FIN).
        /// For example, a stream closure or final piece of a fragmented message.
        const FIN = 0b0000_1000;
        /// Indicates that this frame is an open operation.
        /// Used together with REQUEST/RESPONSE to establish new logical streams.
        const OPEN = 0b0001_0000;
        /// Indicates that this frame represents a reset (abnormal termination) of a stream.
        /// Typically sent as a response to reject an open request or forcibly terminate a stream.
        const RESET = 0b0010_0000;
        /// Indicates that this frame is a close operation (graceful termination) of a stream.
        /// Used together with REQUEST/RESPONSE to properly close a logical stream.
        const CLOSE = 0b0100_0000;
    }
}

impl FrameFlags {
    /// Create a new `FrameFlags` with no flags set
    pub fn new() -> Self {
        Self::from_bits_truncate(FrameFlags::EMPTY.bits())
    }
    /// Create a new `FrameFlags` with open request flags.
    pub fn open_request() -> Self {
        let mut flags = Self::new();
        flags.set_flag(FrameFlags::REQUEST);
        flags.set_flag(FrameFlags::OPEN);
        flags
    }
    /// Create a new `FrameFlags` with open response flags.
    pub fn open_response() -> Self {
        let mut flags = Self::new();
        flags.set_flag(FrameFlags::RESPONSE);
        flags.set_flag(FrameFlags::OPEN);
        flags
    }
    /// Create a new `FrameFlags` with open reset flags.
    pub fn open_reset() -> Self {
        let mut flags = Self::new();
        flags.set_flag(FrameFlags::RESPONSE);
        flags.set_flag(FrameFlags::OPEN);
        flags.set_flag(FrameFlags::RESET);
        flags
    }
    /// Create a new `FrameFlags` with close request flags.
    pub fn close_request() -> Self {
        let mut flags = Self::new();
        flags.set_flag(FrameFlags::REQUEST);
        flags.set_flag(FrameFlags::CLOSE);
        flags
    }
    /// Create a new `FrameFlags` with close response flags.
    pub fn close_response() -> Self {
        let mut flags = Self::new();
        flags.set_flag(FrameFlags::RESPONSE);
        flags.set_flag(FrameFlags::CLOSE);
        flags
    }
    /// Check if the frame has no flags set
    pub fn is_none(&self) -> bool {
        self.is_empty()
    }
    /// Check if the frame is a request
    pub fn is_request(&self) -> bool {
        self.contains(FrameFlags::REQUEST)
    }
    /// Check if the frame is a response
    pub fn is_response(&self) -> bool {
        self.contains(FrameFlags::RESPONSE)
    }
    /// Check if the frame is a fragmented frame
    pub fn is_fragmented(&self) -> bool {
        self.contains(FrameFlags::FRAGMENTED)
    }
    /// Check if the frame is a open handshake frame
    pub fn is_open(&self) -> bool {
        self.contains(FrameFlags::OPEN)
    }
    /// Check if the frame is a FIN frame
    pub fn is_fin(&self) -> bool {
        self.contains(FrameFlags::FIN)
    }
    /// Check if the frame is a RESET frame
    pub fn is_reset(&self) -> bool {
        self.contains(FrameFlags::RESET)
    }
    /// Check if this frame is a open request.
    pub fn is_open_request(self) -> bool {
        self == (FrameFlags::REQUEST | FrameFlags::OPEN)
    }
    /// Check if this frame is a open response.
    pub fn is_open_response(self) -> bool {
        self == (FrameFlags::RESPONSE | FrameFlags::OPEN)
    }
    /// Check if this frame is a open reset.
    pub fn is_open_reset(self) -> bool {
        self == (FrameFlags::RESPONSE | FrameFlags::OPEN | FrameFlags::RESET)
    }
    /// Check if this frame is a close request.
    pub fn is_close_request(self) -> bool {
        self == (FrameFlags::REQUEST | FrameFlags::CLOSE)
    }
    /// Check if this frame is a close response.
    pub fn is_close_response(self) -> bool {
        self == (FrameFlags::RESPONSE | FrameFlags::CLOSE)
    }
    /// Set a flag
    pub fn set_flag(&mut self, flag: FrameFlags) {
        self.insert(flag);
    }
    /// Unset a flag
    pub fn unset_flag(&mut self, flag: FrameFlags) {
        self.remove(flag);
    }
    /// Set flags from u8 value
    pub fn from_u8(&mut self, value: u8) {
        Self::from_bits_truncate(value);
    }
    /// Get flags as u8 value
    pub fn as_u8(&self) -> u8 {
        self.bits()
    }
}

impl Serialize for FrameFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize the underlying bits (u8)
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for FrameFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize the bits and convert them into FrameFlags
        let bits = u8::deserialize(deserializer)?;
        FrameFlags::from_bits(bits)
            .ok_or_else(|| serde::de::Error::custom("Invalid FrameFlags bits"))
    }
}

#[repr(u8)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Control = 0,
    Data = 1,
    Meta = 2,
    FileMeta = 3,
    FileData = 4,
    ContentRequest = 5,
    Reserved(u8),
}

impl From<u8> for FrameType {
    fn from(value: u8) -> Self {
        match value {
            0 => FrameType::Control,
            1 => FrameType::Data,
            2 => FrameType::Meta,
            3 => FrameType::FileMeta,
            4 => FrameType::FileData,
            5 => FrameType::ContentRequest,
            other => FrameType::Reserved(other),
        }
    }
}

impl From<FrameType> for u8 {
    fn from(ft: FrameType) -> Self {
        match ft {
            FrameType::Control => 0,
            FrameType::Data => 1,
            FrameType::Meta => 2,
            FrameType::FileMeta => 3,
            FrameType::FileData => 4,
            FrameType::ContentRequest => 5,
            FrameType::Reserved(other) => other,
        }
    }
}

/// Represents the header of a frame, containing metadata such as stream ID, operation ID, and flags.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// Identifier for a stream within a particular connection
    pub stream_id: StreamId,
    /// A unique identifier for the operation, allowing tracking of individual send/receive operations.
    /// This ID can be used for error handling, retransmissions, and associating responses with requests.
    pub operation_id: OperationId,
    /// Flags providing additional information about the frame.
    /// Each bit in this field can represent a specific flag, such as stream control, request/response or fragmentation
    pub flags: FrameFlags,
    pub frame_type: FrameType,
    pub payload_len: u32,
}

impl FrameHeader {
    /// Create a new `FrameHeader` with the required fields.
    pub fn new(stream_id: StreamId, operation_id: OperationId, flags: FrameFlags, frame_type: FrameType) -> Self {
        Self {
            stream_id,
            operation_id,
            flags,
            frame_type: frame_type,
            payload_len: 0,
        }
    }
    pub fn empty() -> Self {
        Self {
            stream_id: StreamId(0),
            operation_id: OperationId(0),
            flags: FrameFlags::EMPTY,
            frame_type: FrameType::Data,
            payload_len: 0,
        }
    }
    /// Sets a flag.
    pub fn set_flag(&mut self, flag: FrameFlags) {
        self.flags.set_flag(flag);
    }
    /// Unsets a flag.
    pub fn unset_flag(&mut self, flag: FrameFlags) {
        self.flags.unset_flag(flag);
    }
    /// Checks if a flag is set.
    pub fn has_flag(&self, flag: FrameFlags) -> bool {
        self.flags.contains(flag)
    }
    /// Check if the frame is a final frame of an operation
    pub fn is_fin(&self) -> bool {
        self.flags.is_fin()
    }
    /// Check if the frame is a request
    pub fn is_request(&self) -> bool {
        self.flags.is_request()
    }
    /// Check if the frame is a response
    pub fn is_response(&self) -> bool {
        self.flags.is_response()
    }
    /// Check if the frame is a fragmented frame
    pub fn is_fragmented(&self) -> bool {
        self.flags.is_fragmented()
    }
    /// Set the frame as a request
    pub fn set_request(&mut self) {
        self.flags.set_flag(FrameFlags::REQUEST);
    }
    /// Set the frame as a response
    pub fn set_response(&mut self) {
        self.flags.set_flag(FrameFlags::RESPONSE);
    }
    /// Set the frame as a fragmented frame
    pub fn set_fragmented(&mut self) {
        self.flags.set_flag(FrameFlags::FRAGMENTED);
    }
}

/// The frame structure that is sent between the peers
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    /// The header of the frame, containing metadata such as stream ID, operation ID, and flags.
    pub header: FrameHeader,
    /// The payload of the frame. This can be any data that needs to be sent between the peers.
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame with default empty header and no payload.
    pub fn empty() -> Self {
        Self {
            header: FrameHeader::empty(),
            payload: Bytes::new(),
        }
    }
    /// Starts building a new `Frame` using `FrameBuilder`.
    pub fn builder() -> FrameBuilder {
        FrameBuilder::new()
    }
    /// Convert the frame to `Bytes`
    pub fn to_bytes(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_LEN + self.payload.len());
        buf.put_u32(self.header.stream_id.0);
        buf.put_u32(self.header.operation_id.0);
        buf.put_u8(self.header.flags.bits());
        buf.put_u8(self.header.frame_type.into());
        buf.put_u32(self.payload.len() as u32);
        buf.extend_from_slice(&self.payload);
        Ok(buf.freeze())
    }
    /// Convert `Bytes` to a frame
    pub fn from_bytes(mut bytes: Bytes) -> Result<Self> {
        if bytes.len() < FRAME_HEADER_LEN {
            anyhow::bail!("Not enough bytes to read frame header");
        }
        let stream_id = StreamId(bytes.get_u32());
        let operation_id = OperationId(bytes.get_u32());
        let flags = FrameFlags::from_bits_truncate(bytes.get_u8());
        let frame_type = FrameType::from(bytes.get_u8());
        let payload_len = bytes.get_u32();
        Ok(Self {
            header: FrameHeader { stream_id, operation_id, flags, frame_type, payload_len },
            payload: bytes,
        })
    }
    /// Convert the frame to a byte array
    pub fn to_byte_array(&self) -> Result<Vec<u8>> {
        self.to_bytes()
            .and_then(|bytes| Ok(bytes.to_vec()))
            .map_err(|e| e)
    }
    /// Convert a byte array to a frame
    pub fn from_byte_array(bytes: &[u8]) -> Result<Self> {
        let bytes = Bytes::copy_from_slice(bytes);
        Self::from_bytes(bytes)
            .map_err(|e| e)
    }
    /// Get legnth of the frame
    pub fn len(&self) -> usize {
        FRAME_HEADER_LEN + self.payload.len()
    }
    /// Get length of payload
    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }
}

pub struct FrameBuilder {
    /// The header of the frame, containing metadata such as stream ID, operation ID, and flags.
    pub header: FrameHeader,
    /// The payload of the frame. This can be any data that needs to be sent between the peers.
    pub payload: Bytes,
}

impl FrameBuilder {
    /// Creates a new `FrameHeaderBuilder` with the required fields.
    pub fn new() -> Self {
        Self {
            header: FrameHeader::empty(),
            payload: Bytes::new(),
        }
    }
    /// Sets the fin flag.
    pub fn with_fin(mut self, fin: bool) -> Self {
        if fin {
            self.header.flags.set_flag(FrameFlags::FIN);
        } else {
            self.header.flags.unset_flag(FrameFlags::FIN);
        }
        self
    }
    /// Sets the stream ID.
    pub fn with_stream_id(mut self, stream_id: StreamId) -> Self {
        self.header.stream_id = stream_id;
        self
    }
    /// Sets the operation ID.
    pub fn with_operation_id(mut self, operation_id: OperationId) -> Self {
        self.header.operation_id = operation_id;
        self
    }
    /// Sets the flags.
    pub fn with_flags(mut self, flags: FrameFlags) -> Self {
        self.header.flags = flags;
        self
    }
    /// Sets the frame type.
    pub fn with_frame_type(mut self, frame_type: FrameType) -> Self {
        self.header.frame_type = frame_type;
        self
    }
    /// Sets the flags as a request.
    pub fn as_request(mut self) -> Self {
        self.header.flags.set_flag(FrameFlags::REQUEST);
        self
    }
    /// Sets the flags as a response.
    pub fn as_response(mut self) -> Self {
        self.header.flags.set_flag(FrameFlags::RESPONSE);
        self
    }
    /// Sets the flags as a fragmented.
    pub fn as_fragmented(mut self) -> Self {
        self.header.flags.set_flag(FrameFlags::FRAGMENTED);
        self
    }
    /// Sets the payload.
    pub fn with_payload(mut self, payload: Bytes) -> Self {
        self.payload = payload;
        self
    }
    /// Builds the `Frame`.
    pub fn build(self) -> Frame {
        Frame {
            header: self.header,
            payload: self.payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_frame_to_bytes_and_back() {
        let frame = Frame::builder()
            .with_stream_id(StreamId(123))
            .with_operation_id(OperationId(456))
            .with_flags(FrameFlags::FIN)
            .with_frame_type(FrameType::Data)
            .with_payload(Bytes::from_static(b"test-data"))
            .build();

        let bytes = frame.to_bytes().expect("to_bytes failed");
        let parsed = Frame::from_bytes(bytes).expect("from_bytes failed");

        assert_eq!(frame.header.stream_id, parsed.header.stream_id);
        assert_eq!(frame.header.operation_id, parsed.header.operation_id);
        assert_eq!(frame.header.flags, parsed.header.flags);
        assert_eq!(frame.header.frame_type, parsed.header.frame_type);
        assert_eq!(frame.payload, parsed.payload);
    }

    #[test]
    fn test_frame_flags_open_close() {
        let open_req = FrameFlags::open_request();
        assert!(open_req.is_open_request());
        assert!(open_req.is_open());
        assert!(open_req.is_request());

        let open_res = FrameFlags::open_response();
        assert!(open_res.is_open_response());
        assert!(open_res.is_open());
        assert!(open_res.is_response());

        let open_rst = FrameFlags::open_reset();
        assert!(open_rst.is_open_reset());
        assert!(open_rst.is_open());
        assert!(open_rst.is_response());
        assert!(open_rst.is_reset());

        let close_req = FrameFlags::close_request();
        assert!(close_req.is_close_request());
        assert!(close_req.is_request());

        let close_res = FrameFlags::close_response();
        assert!(close_res.is_close_response());
        assert!(close_res.is_response());
    }
}
