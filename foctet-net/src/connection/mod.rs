pub mod endpoint;
pub mod tcp;
pub mod quic;

use anyhow::Result;
use quic::QuicStream;
use tcp::TlsTcpStream;
use std::{collections::HashMap, net::SocketAddr, path::Path, sync::Arc};
use foctet_core::{frame::{Frame, StreamId}, node::ConnectionId};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
pub struct StreamMap {
    streams: Arc<RwLock<HashMap<StreamId, Arc<Mutex<NetworkStream>>>>>,
}

impl StreamMap {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_stream(&self, stream_id: StreamId, stream: Arc<Mutex<NetworkStream>>) {
        let mut streams = self.streams.write().await;
        streams.insert(stream_id, stream);
    }

    pub async fn get_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<NetworkStream>>> {
        let streams = self.streams.read().await;
        streams.get(stream_id).cloned()
    }

    pub async fn remove_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<NetworkStream>>> {
        let mut streams = self.streams.write().await;
        streams.remove(stream_id)
    }
}

#[derive(Debug)]
pub struct Session {
    pub connection_id: ConnectionId,
    pub stream_map: StreamMap,
}

impl Session {
    pub fn new(connection_id: ConnectionId) -> Self {
        Self {
            connection_id,
            stream_map: StreamMap::new(),
        }
    }

    pub async fn add_stream(&self, stream_id: StreamId, stream: Arc<Mutex<NetworkStream>>) {
        self.stream_map.add_stream(stream_id, stream).await;
    }

    pub async fn get_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<NetworkStream>>> {
        self.stream_map.get_stream(stream_id).await
    }

    pub async fn remove_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<NetworkStream>>> {
        self.stream_map.remove_stream(stream_id).await
    }
}

#[allow(async_fn_in_trait)]
pub trait FoctetStream {
    // Connection ID
    fn connection_id(&self) -> ConnectionId;
    
    /// Stream ID
    fn stream_id(&self) -> StreamId;

    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8]) -> Result<()>;

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<()>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self) -> Result<Frame>;

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &Path) -> Result<()>;

    /// Receive a file over the stream
    async fn receive_file(&mut self, file_path: &Path) -> Result<u64>;

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()>;

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool;

    /// Returns the remote address of the connection.
    fn remote_address(&self) -> SocketAddr;
}

#[derive(Debug)]
pub enum NetworkStream {
    Quic(QuicStream),
    Tcp(TlsTcpStream),
}

#[allow(async_fn_in_trait)]
impl FoctetStream for NetworkStream {
    fn connection_id(&self) -> ConnectionId {
        match self {
            NetworkStream::Quic(stream) => stream.connection_id(),
            NetworkStream::Tcp(stream) => stream.connection_id(),
        }
    }
    fn stream_id(&self) -> StreamId {
        match self {
            NetworkStream::Quic(stream) => stream.stream_id(),
            NetworkStream::Tcp(stream) => stream.stream_id(),
        }
    }

    async fn send_data(&mut self, data: &[u8]) -> Result<()> {
        match self {
            NetworkStream::Quic(stream) => stream.send_data(data).await,
            NetworkStream::Tcp(stream) => stream.send_data(data).await,
        }
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize> {
        match self {
            NetworkStream::Quic(stream) => stream.receive_data(buffer).await,
            NetworkStream::Tcp(stream) => stream.receive_data(buffer).await,
        }
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        match self {
            NetworkStream::Quic(stream) => stream.send_frame(frame).await,
            NetworkStream::Tcp(stream) => stream.send_frame(frame).await,
        }
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        match self {
            NetworkStream::Quic(stream) => stream.receive_frame().await,
            NetworkStream::Tcp(stream) => stream.receive_frame().await,
        }
    }

    async fn send_file(&mut self, file_path: &Path) -> Result<()> {
        match self {
            NetworkStream::Quic(stream) => stream.send_file(file_path).await,
            NetworkStream::Tcp(stream) => stream.send_file(file_path).await,
        }
    }

    async fn receive_file(&mut self, file_path: &Path) -> Result<u64> {
        match self {
            NetworkStream::Quic(stream) => stream.receive_file(file_path).await,
            NetworkStream::Tcp(stream) => stream.receive_file(file_path).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            NetworkStream::Quic(stream) => stream.close().await,
            NetworkStream::Tcp(stream) => stream.close().await,
        }
    }

    fn is_closed(&self) -> bool {
        match self {
            NetworkStream::Quic(stream) => stream.is_closed(),
            NetworkStream::Tcp(stream) => stream.is_closed(),
        }
    }

    fn remote_address(&self) -> SocketAddr {
        match self {
            NetworkStream::Quic(stream) => stream.remote_address(),
            NetworkStream::Tcp(stream) => stream.remote_address(),
        }
    }
}
