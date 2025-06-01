use anyhow::Result;
use bytes::{Bytes, BytesMut};
use foctet_core::{frame::Frame, stream::StreamId};
use std::{future::Future, path::Path};
use tokio::io::{AsyncRead, AsyncWrite};
use std::pin::Pin;
use std::task::{Context, Poll};
use super::{quic::stream::{QuicRecvStream, QuicSendStream, QuicStream}, tcp::stream::{LogicalTcpRecvStream, LogicalTcpSendStream, LogicalTcpStream}};

pub trait SendStreamHandle: AsyncWrite + Unpin + Send {
    fn stream_id(&self) -> StreamId;
    fn send(&mut self, data: Bytes) -> impl Future<Output = Result<()>> + Send;
    fn send_frame(&mut self, frame: Frame) -> impl Future<Output = Result<()>> + Send;
    fn send_file(&mut self, file_path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn close(&mut self) -> impl Future<Output = Result<()>> + Send;
}

pub trait RecvStreamHandle: AsyncRead + Unpin + Send {
    fn stream_id(&self) -> StreamId;
    fn receive(&mut self) -> impl Future<Output = Result<BytesMut>> + Send;
    fn receive_frame(&mut self) -> impl Future<Output = Result<Frame>> + Send;
    fn receive_file(&mut self, file_path: &Path, len: usize) -> impl Future<Output = Result<usize>> + Send;
    fn close(&mut self) -> impl Future<Output = Result<()>> + Send;
}

pub trait StreamHandle: AsyncRead + AsyncWrite + Unpin + Send {
    fn stream_id(&self) -> StreamId;
    fn send(&mut self, data: Bytes) -> impl Future<Output = Result<()>> + Send;
    fn receive(&mut self) -> impl Future<Output = Result<BytesMut>> + Send;
    fn send_frame(&mut self, frame: Frame) -> impl Future<Output = Result<()>> + Send;
    fn receive_frame(&mut self) -> impl Future<Output = Result<Frame>> + Send;
    fn send_file(&mut self, file_path: &Path) -> impl Future<Output = Result<()>> + Send;
    fn receive_file(&mut self, file_path: &Path, len: usize) -> impl Future<Output = Result<usize>> + Send;
    fn close(&mut self) -> impl Future<Output = Result<()>> + Send;
}

pub enum SendStream {
    Quic(QuicSendStream),
    Tcp(LogicalTcpSendStream),
}

impl SendStream {
    pub fn stream_id(&self) -> StreamId {
        match self {
            SendStream::Quic(send) => send.stream_id(),
            SendStream::Tcp(send) => send.stream_id(),
        }
    }

    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        match self {
            SendStream::Quic(send) => send.send(data).await,
            SendStream::Tcp(send) => send.send(data).await,
        }
    }

    pub async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        match self {
            SendStream::Quic(send) => send.send_frame(frame).await,
            SendStream::Tcp(send) => send.send_frame(frame).await,
        }
    }

    pub async fn send_file(&mut self, file_path: &Path) -> Result<()> {
        match self {
            SendStream::Quic(send) => send.send_file(file_path).await,
            SendStream::Tcp(send) => send.send_file(file_path).await,
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        match self {
            SendStream::Quic(send) => send.close().await,
            SendStream::Tcp(send) => send.close().await,
        }
    }
}

impl AsyncWrite for SendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            SendStream::Quic(send) => Pin::new(send).poll_write(cx, buf),
            SendStream::Tcp(send) => Pin::new(send).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SendStream::Quic(send) => Pin::new(send).poll_flush(cx),
            SendStream::Tcp(send) => Pin::new(send).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SendStream::Quic(send) => Pin::new(send).poll_shutdown(cx),
            SendStream::Tcp(send) => Pin::new(send).poll_shutdown(cx),
        }
    }
}

pub enum RecvStream {
    Quic(QuicRecvStream),
    Tcp(LogicalTcpRecvStream),
}

impl RecvStream {
    pub fn stream_id(&self) -> StreamId {
        match self {
            RecvStream::Quic(recv) => recv.stream_id(),
            RecvStream::Tcp(recv) => recv.stream_id(),
        }
    }

    pub async fn receive(&mut self) -> Result<BytesMut> {
        match self {
            RecvStream::Quic(recv) => recv.receive().await,
            RecvStream::Tcp(recv) => recv.receive().await,
        }
    }

    pub async fn receive_frame(&mut self) -> Result<Frame> {
        match self {
            RecvStream::Quic(recv) => recv.receive_frame().await,
            RecvStream::Tcp(recv) => recv.receive_frame().await,
        }
    }

    pub async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        match self {
            RecvStream::Quic(recv) => recv.receive_file(file_path, len).await,
            RecvStream::Tcp(recv) => recv.receive_file(file_path, len).await,
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        match self {
            RecvStream::Quic(recv) => recv.close().await,
            RecvStream::Tcp(recv) => recv.close().await,
        }
    }
}

impl AsyncRead for RecvStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            RecvStream::Quic(recv) => Pin::new(recv).poll_read(cx, buf),
            RecvStream::Tcp(recv) => Pin::new(recv).poll_read(cx, buf),
        }
    }
}

pub enum Stream {
    Quic(QuicStream),
    Tcp(LogicalTcpStream),
}

impl Stream {
    pub fn stream_id(&self) -> StreamId {
        match self {
            Stream::Quic(stream) => stream.stream_id(),
            Stream::Tcp(stream) => stream.stream_id(),
        }
    }
    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        match self {
            Stream::Quic(stream) => stream.send(data).await,
            Stream::Tcp(stream) => stream.send(data).await,
        }
    }
    pub async fn receive(&mut self) -> Result<BytesMut> {
        match self {
            Stream::Quic(stream) => stream.receive().await,
            Stream::Tcp(stream) => stream.receive().await,
        }
    }
    pub async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        match self {
            Stream::Quic(stream) => stream.send_frame(frame).await,
            Stream::Tcp(stream) => stream.send_frame(frame).await,
        }
    }
    pub async fn receive_frame(&mut self) -> Result<Frame> {
        match self {
            Stream::Quic(stream) => stream.receive_frame().await,
            Stream::Tcp(stream) => stream.receive_frame().await,
        }
    }
    pub async fn send_file(&mut self, file_path: &Path) -> Result<()> {
        match self {
            Stream::Quic(stream) => stream.send_file(file_path).await,
            Stream::Tcp(stream) => stream.send_file(file_path).await,
        }
    }
    pub async fn receive_file(&mut self, file_path: &Path, len: usize) -> Result<usize> {
        match self {
            Stream::Quic(stream) => stream.receive_file(file_path, len).await,
            Stream::Tcp(stream) => stream.receive_file(file_path, len).await,
        }
    }
    pub async fn close(&mut self) -> Result<()> {
        match self {
            Stream::Quic(stream) => stream.close().await,
            Stream::Tcp(stream) => stream.close().await,
        }
    }
    pub fn split(self) -> (SendStream, RecvStream) {
        match self {
            Stream::Quic(stream) => {
                let (send_stream, recv_stream) = stream.split();
                (SendStream::Quic(send_stream), RecvStream::Quic(recv_stream))
            },
            Stream::Tcp(stream) => {
                let (send_stream, recv_stream) = stream.split();
                (SendStream::Tcp(send_stream), RecvStream::Tcp(recv_stream))
            },
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Quic(stream) => Pin::new(stream).poll_read(cx, buf),
            Stream::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Stream::Quic(stream) => Pin::new(stream).poll_write(cx, buf),
            Stream::Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Quic(stream) => Pin::new(stream).poll_flush(cx),
            Stream::Tcp(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Quic(stream) => Pin::new(stream).poll_shutdown(cx),
            Stream::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
