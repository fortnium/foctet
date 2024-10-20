pub mod endpoint;
pub mod tcp;
pub mod quic;

use anyhow::Result;
use foctet_core::frame::{ContentId, Frame};

pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[allow(async_fn_in_trait)]
pub trait FoctetStream {
    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8], content_id: Option<ContentId>) -> Result<()>;

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>, content_id: Option<ContentId>) -> Result<usize>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<()>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self, content_id: Option<ContentId>) -> Result<Frame>;

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<()>;

    /// Receive a file over the stream
    async fn receive_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<u64>;

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()>;
}
