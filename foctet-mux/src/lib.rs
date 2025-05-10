// Stream module
pub mod stream;
// Session module
pub mod session;

// Stream ID type
pub use foctet_core::stream::StreamId;
// Session ID type
pub use foctet_core::connection::SessionId;

pub use crate::{session::Session, stream::LogicalStream};

// Latest Protocol Version
pub const PROTOCOL_VERSION: u8 = 0;
// The 0 ID is reserved to represent the session.
pub const RESERVED_STREAM_ID: StreamId = StreamId(0);
