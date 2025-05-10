use anyhow::Result;
use bytes::{Bytes, BytesMut};
use foctet_core::{frame::Frame, stream::StreamId};
use std::{future::Future, path::Path};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{quic::stream::QuicStream, tcp::stream::LogicalTcpStream};

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
}
