use crate::transport::stream::{RecvStreamHandle, SendStreamHandle, StreamHandle};
use anyhow::Result;
use bytes::{Bytes, BytesMut};
use foctet_core::frame::{Frame, FrameType};
use foctet_core::stream::StreamId;
use foctet_mux::stream::{LogicalStreamReader, LogicalStreamWriter};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug)]
pub struct LogicalTcpSendStream {
    framed_writer: LogicalStreamWriter,
    stream_id: StreamId,
    pub write_buffer_size: usize,
}

impl LogicalTcpSendStream {
    pub fn new(
        stream_id: StreamId,
        send_stream: LogicalStreamWriter,
        write_buffer_size: usize,
    ) -> Self {
        Self {
            stream_id: stream_id,
            framed_writer: send_stream,
            write_buffer_size,
        }
    }
}

impl AsyncWrite for LogicalTcpSendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_shutdown(cx)
    }
}

impl SendStreamHandle for LogicalTcpSendStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    async fn send(&mut self, data: Bytes) -> Result<()> {
        self.framed_writer.send_bytes(data).await?;
        Ok(())
    }
    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        self.framed_writer.send_frame(frame).await?;
        Ok(())
    }
    async fn send_file(&mut self, file_path: &Path) -> Result<()> {
        // Send file data
        let mut file = File::open(file_path).await?;
        let mut buf = vec![0u8; self.write_buffer_size];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            let chunk = Bytes::copy_from_slice(&buf[..n]);
            let frame: Frame = Frame::builder()
                .with_stream_id(self.stream_id)
                .with_fin(false)
                .with_frame_type(FrameType::Data)
                .with_payload(chunk)
                .build();
            self.framed_writer.send_frame(frame).await?;
        }
        Ok(())
    }
    async fn close(&mut self) -> Result<()> {
        self.framed_writer.close().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct LogicalTcpRecvStream {
    framed_reader: LogicalStreamReader,
    stream_id: StreamId,
    pub read_buffer_size: usize,
}

impl LogicalTcpRecvStream {
    pub fn new(
        stream_id: StreamId,
        recv_stream: LogicalStreamReader,
        read_buffer_size: usize,
    ) -> Self {
        Self {
            stream_id: stream_id,
            framed_reader: recv_stream,
            read_buffer_size,
        }
    }
}

impl AsyncRead for LogicalTcpRecvStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_reader).poll_read(cx, buf)
    }
}

impl RecvStreamHandle for LogicalTcpRecvStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    async fn receive(&mut self) -> Result<BytesMut> {
        match self.framed_reader.recv_bytes().await {
            Ok(bytes) => Ok(bytes.into()),
            Err(e) => Err(e.into()),
        }
    }
    async fn receive_frame(&mut self) -> Result<Frame> {
        match self.framed_reader.recv_frame().await {
            Ok(frame) => Ok(frame),
            Err(e) => Err(e.into()),
        }
    }

    async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        // Receive file data
        let mut total_bytes: usize = 0;
        let mut file = File::create(file_path).await?;

        loop {
            let frame = self.framed_reader.recv_frame().await?;
            file.write_all(&frame.payload).await?;
            total_bytes += frame.payload.len();
            if total_bytes == len {
                break;
            }
        }
        file.flush().await?;
        Ok(total_bytes)
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct LogicalTcpStream {
    framed_writer: LogicalStreamWriter,
    framed_reader: LogicalStreamReader,
    stream_id: StreamId,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
}

impl LogicalTcpStream {
    /// Create new `LogicalTcpStream`
    pub fn new(
        stream_id: StreamId,
        send_stream: LogicalStreamWriter,
        recv_stream: LogicalStreamReader,
        write_buffer_size: usize,
        read_buffer_size: usize,
    ) -> Self {
        Self {
            framed_writer: send_stream,
            framed_reader: recv_stream,
            stream_id,
            write_buffer_size,
            read_buffer_size,
        }
    }

    /// Set the write buffer size
    pub fn set_write_buffer_size(&mut self, size: usize) {
        self.write_buffer_size = size;
    }

    /// Set the read buffer size
    pub fn set_read_buffer_size(&mut self, size: usize) {
        self.read_buffer_size = size;
    }

    /// Split the stream into send and receive streams
    pub fn split(self) -> (LogicalTcpSendStream, LogicalTcpRecvStream) {
        let send_stream =
            LogicalTcpSendStream::new(self.stream_id, self.framed_writer, self.write_buffer_size);
        let recv_stream =
            LogicalTcpRecvStream::new(self.stream_id, self.framed_reader, self.read_buffer_size);
        (send_stream, recv_stream)
    }

    /// Merge the send and receive streams into a single stream
    pub fn merge(send_stream: LogicalTcpSendStream, recv_stream: LogicalTcpRecvStream) -> Self {
        Self {
            framed_writer: send_stream.framed_writer,
            framed_reader: recv_stream.framed_reader,
            stream_id: send_stream.stream_id,
            write_buffer_size: send_stream.write_buffer_size,
            read_buffer_size: recv_stream.read_buffer_size,
        }
    }
}

impl AsyncRead for LogicalTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_reader).poll_read(cx, buf)
    }
}

impl AsyncWrite for LogicalTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer).poll_shutdown(cx)
    }
}

impl StreamHandle for LogicalTcpStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    async fn send(&mut self, data: Bytes) -> Result<()> {
        self.framed_writer.send_bytes(data).await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<BytesMut> {
        match self.framed_reader.recv_bytes().await {
            Ok(bytes) => Ok(bytes.into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        self.framed_writer.send_frame(frame).await?;
        Ok(())
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        match self.framed_reader.recv_frame().await {
            Ok(frame) => Ok(frame),
            Err(e) => Err(e.into()),
        }
    }

    async fn send_file(&mut self, file_path: &Path) -> Result<()> {
        // Send file data
        let mut file = File::open(file_path).await?;
        let mut buf = vec![0u8; self.write_buffer_size];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            let chunk = Bytes::copy_from_slice(&buf[..n]);
            let frame: Frame = Frame::builder()
                .with_stream_id(self.stream_id)
                .with_fin(false)
                .with_frame_type(FrameType::Data)
                .with_payload(chunk)
                .build();
            self.framed_writer.send_frame(frame).await?;
        }
        Ok(())
    }

    async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        // Receive file data
        let mut total_bytes: usize = 0;
        let mut file = File::create(file_path).await?;

        loop {
            let frame = self.framed_reader.recv_frame().await?;
            file.write_all(&frame.payload).await?;
            total_bytes += frame.payload.len();
            if total_bytes == len {
                break;
            }
        }
        file.flush().await?;
        Ok(total_bytes)
    }

    async fn close(&mut self) -> Result<()> {
        self.framed_writer.close().await?;
        Ok(())
    }
}
