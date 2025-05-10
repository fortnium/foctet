use tokio_util::codec::{Encoder, Decoder};
use bytes::{BytesMut, Buf, BufMut};
use std::io;
use crate::frame::{Frame, FrameFlags, FrameHeader, FrameType, FRAME_HEADER_LEN};

/// A codec for encoding and decoding Frames.
pub struct FrameCodec {
    state: DecodeState,
}

enum DecodeState {
    ReadingHeader,
    ReadingPayload {
        header: FrameHeader,
        remaining: usize,
    },
}

impl FrameCodec {
    pub fn new() -> Self {
        Self {
            state: DecodeState::ReadingHeader,
        }
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Frame>, io::Error> {
        loop {
            match &mut self.state {
                DecodeState::ReadingHeader => {
                    if src.len() < FRAME_HEADER_LEN {
                        return Ok(None);
                    }

                    let mut header_bytes = src.split_to(FRAME_HEADER_LEN);
                    let stream_id = header_bytes.get_u32();
                    let operation_id = header_bytes.get_u32();
                    let flags = FrameFlags::from_bits_truncate(header_bytes.get_u8());
                    let frame_type = FrameType::from(header_bytes.get_u8());
                    let payload_len = header_bytes.get_u32() as usize;

                    let header = FrameHeader {
                        stream_id: crate::stream::StreamId(stream_id),
                        operation_id: crate::stream::OperationId(operation_id),
                        flags,
                        frame_type,
                        payload_len: payload_len as u32,
                    };

                    self.state = DecodeState::ReadingPayload {
                        header,
                        remaining: payload_len,
                    }
                }

                DecodeState::ReadingPayload { header, remaining } => {
                    if src.len() < *remaining {
                        return Ok(None);
                    }

                    let payload = src.split_to(*remaining).freeze();
                    let frame = Frame {
                        header: header.clone(),
                        payload,
                    };

                    self.state = DecodeState::ReadingHeader;
                    return Ok(Some(frame));
                }
            }
        }
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), io::Error> {
        dst.reserve(FRAME_HEADER_LEN + frame.payload.len());

        dst.put_u32(frame.header.stream_id.0);
        dst.put_u32(frame.header.operation_id.0);
        dst.put_u8(frame.header.flags.bits());
        dst.put_u8(frame.header.frame_type.into());
        dst.put_u32(frame.payload.len() as u32);

        dst.extend_from_slice(&frame.payload);

        Ok(())
    }
}
