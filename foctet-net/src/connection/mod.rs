pub mod endpoint;
pub mod tcp;
pub mod quic;

use anyhow::Result;
use foctet_core::frame::Frame;

#[allow(async_fn_in_trait)]
pub trait NetworkStream {
    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8]) -> Result<()>;

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<()>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self) -> Result<Frame>;

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<()>;

    /// Receive a file over the stream
    async fn receive_file(&mut self, file_path: &std::path::Path) -> Result<u64>;

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()>;

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool;
}
