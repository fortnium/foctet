pub mod endpoint;
pub mod tcp;
pub mod quic;

use anyhow::Result;
use foctet_core::frame::Frame;

pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[allow(async_fn_in_trait)]
pub trait FoctetStream {
    /// Sends data over the stream
    async fn send_data(&mut self, stream_id: u64, data: &[u8]) -> Result<()>;

    /// Receives data from the stream
    async fn receive_data(&mut self, stream_id: u64, buffer: &mut [u8]) -> Result<usize>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, stream_id: u64, frame: Frame) -> Result<()>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self, stream_id: u64) -> Result<usize>;

    /// Send a file over the stream
    async fn send_file(&self, stream_id: u64, file_path: &std::path::Path) -> Result<()>;

    /// Receive a file over the stream
    async fn receive_file(&self, stream_id: u64, file_path: &std::path::Path) -> Result<u64>;

    /// Gracefully closes the stream.
    fn close(&mut self) -> Result<()>;
}
