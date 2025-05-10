use anyhow::Result;
use bytes::{Bytes, BytesMut};
use foctet_core::frame::Frame;
use foctet_core::stream::StreamId;
use futures::{sink::SinkExt, StreamExt};
use quinn::{RecvStream as QuinnRecvStream, SendStream as QuinnSendStream};
use std::pin::Pin;
use std::{
    path::Path,
    task::{Context, Poll},
};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::transport::stream::{RecvStreamHandle, SendStreamHandle, StreamHandle};

#[derive(Debug)]
pub struct QuicSendStream {
    framed_writer: FramedWrite<QuinnSendStream, LengthDelimitedCodec>,
    stream_id: StreamId,
    pub write_buffer_size: usize,
}

impl QuicSendStream {
    pub fn new(
        stream_id: StreamId,
        send_stream: FramedWrite<QuinnSendStream, LengthDelimitedCodec>,
        write_buffer_size: usize,
    ) -> Self {
        Self {
            stream_id: stream_id,
            framed_writer: send_stream,
            write_buffer_size,
        }
    }
}

impl AsyncWrite for QuicSendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut())
            .poll_write(cx, buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut()).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut()).poll_shutdown(cx)
    }
}

impl SendStreamHandle for QuicSendStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    async fn send(&mut self, data: Bytes) -> Result<()> {
        let mut total_sent = 0;
        while total_sent < data.len() {
            let chunk_size = self.write_buffer_size.min(data.len() - total_sent);
            self.framed_writer
                .send(Bytes::copy_from_slice(
                    &data[total_sent..total_sent + chunk_size],
                ))
                .await?;
            total_sent += chunk_size;
        }
        Ok(())
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        self.framed_writer.send(frame.to_bytes()?).await?;
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
            self.framed_writer
                .send(Bytes::copy_from_slice(&buf[..n]))
                .await?;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.framed_writer.close().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct QuicRecvStream {
    framed_reader: FramedRead<QuinnRecvStream, LengthDelimitedCodec>,
    stream_id: StreamId,
    pub read_buffer_size: usize,
}

impl QuicRecvStream {
    pub fn new(
        stream_id: StreamId,
        recv_stream: FramedRead<QuinnRecvStream, LengthDelimitedCodec>,
        read_buffer_size: usize,
    ) -> Self {
        Self {
            stream_id: stream_id,
            framed_reader: recv_stream,
            read_buffer_size,
        }
    }
}

impl AsyncRead for QuicRecvStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_reader.get_mut()).poll_read(cx, buf)
    }
}

impl RecvStreamHandle for QuicRecvStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    async fn receive(&mut self) -> Result<BytesMut> {
        match self.framed_reader.next().await {
            Some(Ok(bytes)) => Ok(bytes),
            Some(Err(e)) => Err(e.into()),
            None => Err(anyhow::anyhow!("Stream closed")),
        }
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        if let Some(frame_result) = self.framed_reader.next().await {
            let bytes = frame_result?;
            let frame = Frame::from_bytes(bytes.freeze())?;
            Ok(frame)
        } else {
            Err(anyhow::anyhow!("Stream closed"))
        }
    }

    async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        // Receive file data
        let mut total_bytes: usize = 0;
        let mut file = File::create(file_path).await?;

        while let Some(chunk) = self.framed_reader.next().await {
            let data = chunk?;
            if data.is_empty() {
                break;
            }
            file.write_all(&data).await?;
            total_bytes += data.len();

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
pub struct QuicStream {
    framed_writer: FramedWrite<QuinnSendStream, LengthDelimitedCodec>,
    framed_reader: FramedRead<QuinnRecvStream, LengthDelimitedCodec>,
    stream_id: StreamId,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
}

impl QuicStream {
    /// Create new `QuicStream`
    pub fn new(
        stream_id: StreamId,
        send_stream: QuinnSendStream,
        recv_stream: QuinnRecvStream,
        write_buffer_size: usize,
        read_buffer_size: usize,
    ) -> Self {
        Self {
            framed_writer: FramedWrite::new(send_stream, LengthDelimitedCodec::new()),
            framed_reader: FramedRead::new(recv_stream, LengthDelimitedCodec::new()),
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
    pub fn split(self) -> (QuicSendStream, QuicRecvStream) {
        let send_stream =
            QuicSendStream::new(self.stream_id, self.framed_writer, self.write_buffer_size);
        let recv_stream =
            QuicRecvStream::new(self.stream_id, self.framed_reader, self.read_buffer_size);
        (send_stream, recv_stream)
    }

    /// Merge the send and receive streams into a single stream
    pub fn merge(send_stream: QuicSendStream, recv_stream: QuicRecvStream) -> Self {
        Self {
            framed_writer: send_stream.framed_writer,
            framed_reader: recv_stream.framed_reader,
            stream_id: send_stream.stream_id,
            write_buffer_size: send_stream.write_buffer_size,
            read_buffer_size: recv_stream.read_buffer_size,
        }
    }
}

impl AsyncRead for QuicStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_reader.get_mut()).poll_read(cx, buf)
    }
}

impl AsyncWrite for QuicStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut())
            .poll_write(cx, buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut()).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().framed_writer.get_mut()).poll_shutdown(cx)
    }
}

impl StreamHandle for QuicStream {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    async fn send(&mut self, data: Bytes) -> Result<()> {
        let mut total_sent = 0;
        while total_sent < data.len() {
            let chunk_size = self.write_buffer_size.min(data.len() - total_sent);
            self.framed_writer
                .send(Bytes::copy_from_slice(
                    &data[total_sent..total_sent + chunk_size],
                ))
                .await?;
            total_sent += chunk_size;
        }
        Ok(())
    }

    async fn receive(&mut self) -> Result<BytesMut> {
        match self.framed_reader.next().await {
            Some(Ok(bytes)) => Ok(bytes),
            Some(Err(e)) => Err(e.into()),
            None => Err(anyhow::anyhow!("Stream closed")),
        }
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        self.framed_writer.send(frame.to_bytes()?).await?;
        Ok(())
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        if let Some(frame_result) = self.framed_reader.next().await {
            let bytes = frame_result?;
            let frame = Frame::from_bytes(bytes.freeze())?;
            Ok(frame)
        } else {
            Err(anyhow::anyhow!("Stream closed"))
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
            self.framed_writer
                .send(Bytes::copy_from_slice(&buf[..n]))
                .await?;
        }
        Ok(())
    }

    async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        // Receive file data
        let mut total_bytes: usize = 0;
        let mut file = File::create(file_path).await?;

        while let Some(chunk) = self.framed_reader.next().await {
            let data = chunk?;
            if data.is_empty() {
                break;
            }
            file.write_all(&data).await?;
            total_bytes += data.len();

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
