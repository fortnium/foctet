pub mod endpoint;
pub mod quic;
pub mod tcp;
pub mod filter;
pub mod priority;

use anyhow::Result;
use foctet_core::{
    frame::{Frame, OperationId, StreamId},
    node::{ConnectionId, NodeAddr},
};
use quic::{QuicStream, QuicSendStream, QuicRecvStream};
use std::{collections::HashMap, net::SocketAddr, path::Path, sync::Arc, time::Instant};
use tcp::{TlsTcpStream, TlsTcpSendStream, TlsTcpRecvStream};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
pub struct StreamMap {
    //streams: Arc<RwLock<HashMap<StreamId, Arc<Mutex<NetworkStream>>>>>,
    /// The map of send streams
    send_streams: Arc<RwLock<HashMap<StreamId, Arc<Mutex<SendStream>>>>>,
    /// The map of receive streams
    recv_streams: Arc<RwLock<HashMap<StreamId, Arc<Mutex<RecvStream>>>>>,
}

impl StreamMap {
    pub fn new() -> Self {
        Self {
            send_streams: Arc::new(RwLock::new(HashMap::new())),
            recv_streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add SendStream to the map
    pub async fn add_send_stream(&self, stream_id: StreamId, send_stream: Arc<Mutex<SendStream>>) {
        let mut send_streams = self.send_streams.write().await;
        send_streams.insert(stream_id, send_stream);
    }

    /// Add RecvStream to the map
    pub async fn add_recv_stream(&self, stream_id: StreamId, recv_stream: Arc<Mutex<RecvStream>>) {
        let mut recv_streams = self.recv_streams.write().await;
        recv_streams.insert(stream_id, recv_stream);
    }

    // Get SendStream from the map
    pub async fn get_send_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<SendStream>>> {
        let send_streams = self.send_streams.read().await;
        send_streams.get(stream_id).cloned()
    }

    /// Get RecvStream from the map
    pub async fn get_recv_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<RecvStream>>> {
        let recv_streams = self.recv_streams.read().await;
        recv_streams.get(stream_id).cloned()
    }

    /// Remove SendStream from the map
    pub async fn remove_send_stream(&self, stream_id: &StreamId) {
        let mut send_streams = self.send_streams.write().await;
        send_streams.remove(stream_id);
    }

    /// Remove RecvStream from the map
    pub async fn remove_recv_stream(&self, stream_id: &StreamId) {
        let mut recv_streams = self.recv_streams.write().await;
        recv_streams.remove(stream_id);
    }

    /// Remove Stream from the map
    pub async fn remove_stream(&self, stream_id: &StreamId) -> Result<()> {
        self.remove_send_stream(stream_id).await;
        self.remove_recv_stream(stream_id).await;
        Ok(())
    }

    /// Remove all streams from the map
    pub async fn remove_all_streams(&self) {
        let mut send_streams = self.send_streams.write().await;
        send_streams.clear();

        let mut recv_streams = self.recv_streams.write().await;
        recv_streams.clear();
    }
}

#[derive(Debug)]
pub struct Session {
    pub connection_id: ConnectionId,
    pub node_addr: NodeAddr,
    pub quic_connection: Option<quic::QuicConnection>,
    pub stream_map: StreamMap,
    pub last_accessed: Arc<Mutex<Instant>>,
}

impl Session {
    pub fn new(connection_id: ConnectionId, node_addr: NodeAddr) -> Self {
        Self {
            connection_id,
            node_addr,
            quic_connection: None,
            stream_map: StreamMap::new(),
            last_accessed: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn new_with_quic_connection(
        connection_id: ConnectionId,
        node_addr: NodeAddr,
        quic_connection: quic::QuicConnection,
    ) -> Self {
        Self {
            connection_id,
            node_addr,
            quic_connection: Some(quic_connection),
            stream_map: StreamMap::new(),
            last_accessed: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub async fn add_stream(&self, stream_id: StreamId, stream: NetworkStream) {
        self.update_last_accessed().await;
        // Split the stream into send and receive streams
        let (send_stream, recv_stream) = stream.split();
        self.stream_map.add_send_stream(stream_id, Arc::new(Mutex::new(send_stream))).await;
        self.stream_map.add_recv_stream(stream_id, Arc::new(Mutex::new(recv_stream))).await;
    }

    pub async fn get_stream(&self, stream_id: &StreamId) -> Option<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        self.update_last_accessed().await;
        let send_stream = self.get_send_stream(stream_id).await;
        let recv_stream = self.get_recv_stream(stream_id).await;
        if send_stream.is_some() && recv_stream.is_some() {
            Some((send_stream.unwrap(), recv_stream.unwrap()))
        } else {
            None
        }
    }

    pub async fn get_send_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<SendStream>>> {
        self.update_last_accessed().await;
        self.stream_map.get_send_stream(stream_id).await
    }

    pub async fn get_recv_stream(&self, stream_id: &StreamId) -> Option<Arc<Mutex<RecvStream>>> {
        self.update_last_accessed().await;
        self.stream_map.get_recv_stream(stream_id).await
    }

    /// Returns the first available stream.
    /// Which is not closed and not in use (not locked).
    pub async fn get_available_stream(&self) -> Option<(Arc<Mutex<SendStream>>, Arc<Mutex<RecvStream>>)> {
        self.update_last_accessed().await;
        let send_streams = self.stream_map.send_streams.read().await;
        let recv_streams = self.stream_map.recv_streams.read().await;
        for (stream_id, send_stream) in send_streams.iter() {
            if let Ok(send_stream_lock) = send_stream.try_lock() {
                if !send_stream_lock.is_closed() {
                    if let Some(recv_stream) = recv_streams.get(stream_id) {
                        if let Ok(recv_stream_lock) = recv_stream.try_lock() {
                            if !recv_stream_lock.is_closed() {
                                return Some((send_stream.clone(), recv_stream.clone()));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Returns the first available send stream.
    pub async fn get_available_send_stream(&self) -> Option<Arc<Mutex<SendStream>>> {
        self.update_last_accessed().await;
        let send_streams = self.stream_map.send_streams.read().await;
        send_streams.values().find(|stream| {
            match stream.try_lock() {
                Ok(stream_lock) => {
                    !stream_lock.is_closed()
                },
                Err(_) => false,
            }
        }).cloned()
    }

    /// Returns the first available receive stream.
    pub async fn get_available_recv_stream(&self) -> Option<Arc<Mutex<RecvStream>>> {
        self.update_last_accessed().await;
        let recv_streams = self.stream_map.recv_streams.read().await;
        recv_streams.values().find(|stream| {
            match stream.try_lock() {
                Ok(stream_lock) => {
                    !stream_lock.is_closed()
                },
                Err(_) => false,
            }
        }).cloned()
    }

    pub async fn remove_stream(&self, stream_id: &StreamId) -> Result<()> {
        self.stream_map.remove_stream(stream_id).await
    }
    pub async fn update_last_accessed(&self) {
        let mut last_accessed = self.last_accessed.lock().await;
        *last_accessed = Instant::now();
    }
    /// Close all streams in the session.
    pub async fn close(&mut self) {
        // Close all streams
        self.stream_map.remove_all_streams().await;
        // Close the QUIC connection
        if let Some(quic_connection) = &mut self.quic_connection {
            let _ = quic_connection.close().await;
        }
    }
    /// Cleans up unnecessary streams, keeping at least one open stream.
    pub async fn cleanup_streams(&self) {
        // Check available stream
        let available_stream_id: StreamId = if let Some(stream) = self.get_available_send_stream().await {
            stream.lock().await.stream_id()
        }else{
            return;
        };
        // Remove all streams except the 1 available stream.
        let streams = self.stream_map.send_streams.read().await;
        let mut stream_ids: Vec<StreamId> = Vec::new();
        for stream_id in streams.keys() {
            if *stream_id != available_stream_id {
                stream_ids.push(*stream_id);
            }
        }
        for stream_id in stream_ids {
            self.remove_stream(&stream_id).await.unwrap();
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait FoctetStream {
    // Returns the Connection ID
    fn connection_id(&self) -> ConnectionId;

    /// Returns the Stream ID
    fn stream_id(&self) -> StreamId;

    /// Returns the current operation ID.
    fn operation_id(&self) -> OperationId;

    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId>;

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self) -> Result<Frame>;

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &Path) -> Result<OperationId>;

    /// Receive a file over the stream
    async fn receive_file(&mut self, file_path: &Path) -> Result<u64>;

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()>;

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool;

    /// Returns the remote address of the connection.
    fn remote_address(&self) -> SocketAddr;

    /// Splits the stream into send and receive streams.
    fn split(self) -> (SendStream, RecvStream);

    /// Merges a `SendStream` and a `RecvStream` back into a `NetworkStream`.
    fn merge(send_stream: SendStream, recv_stream: RecvStream) -> Result<Self> where Self: Sized;

}

#[allow(async_fn_in_trait)]
pub trait FoctetSendStream {
    // Returns the Connection ID
    fn connection_id(&self) -> ConnectionId;

    /// Returns the Stream ID
    fn stream_id(&self) -> StreamId;

    /// Returns the current operation ID.
    fn operation_id(&self) -> OperationId;

    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId>;

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId>;

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &Path) -> Result<OperationId>;

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()>;

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool;

    /// Returns the remote address of the connection.
    fn remote_address(&self) -> SocketAddr;
}

#[allow(async_fn_in_trait)]
pub trait FoctetRecvStream {
    // Returns the Connection ID
    fn connection_id(&self) -> ConnectionId;

    /// Returns the Stream ID
    fn stream_id(&self) -> StreamId;

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize>;

    /// Receive a frame over the stream
    async fn receive_frame(&mut self) -> Result<Frame>;

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
pub enum SendStream {
    Quic(QuicSendStream),
    Tcp(TlsTcpSendStream),
}

impl FoctetSendStream for SendStream {
    // Returns the Connection ID
    fn connection_id(&self) -> ConnectionId {
        match self {
            SendStream::Quic(stream) => stream.connection_id(),
            SendStream::Tcp(stream) => stream.connection_id(),
        }
    }

    /// Returns the Stream ID
    fn stream_id(&self) -> StreamId {
        match self {
            SendStream::Quic(stream) => stream.stream_id(),
            SendStream::Tcp(stream) => stream.stream_id(),
        }
    }

    /// Returns the current operation ID.
    fn operation_id(&self) -> OperationId {
        match self {
            SendStream::Quic(stream) => stream.operation_id(),
            SendStream::Tcp(stream) => stream.operation_id(),
        }
    }

    /// Sends data over the stream
    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId> {
        match self {
            SendStream::Quic(stream) => stream.send_data(data).await,
            SendStream::Tcp(stream) => stream.send_data(data).await,
        }
    }

    /// Send a frame over the stream
    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
        match self {
            SendStream::Quic(stream) => stream.send_frame(frame).await,
            SendStream::Tcp(stream) => stream.send_frame(frame).await,
        }
    }

    /// Send a file over the stream
    async fn send_file(&mut self, file_path: &Path) -> Result<OperationId> {
        match self {
            SendStream::Quic(stream) => stream.send_file(file_path).await,
            SendStream::Tcp(stream) => stream.send_file(file_path).await,
        }
    }

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()> {
        match self {
            SendStream::Quic(stream) => stream.close().await,
            SendStream::Tcp(stream) => stream.close().await,
        }
    }

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool {
        match self {
            SendStream::Quic(stream) => stream.is_closed(),
            SendStream::Tcp(stream) => stream.is_closed(),
        }
    }

    /// Returns the remote address of the connection.
    fn remote_address(&self) -> SocketAddr {
        match self {
            SendStream::Quic(stream) => stream.remote_address(),
            SendStream::Tcp(stream) => stream.remote_address(),
        }
    }
}

#[derive(Debug)]
pub enum RecvStream {
    Quic(QuicRecvStream),
    Tcp(TlsTcpRecvStream),
}

impl FoctetRecvStream for RecvStream {
    // Returns the Connection ID
    fn connection_id(&self) -> ConnectionId {
        match self {
            RecvStream::Quic(stream) => stream.connection_id(),
            RecvStream::Tcp(stream) => stream.connection_id(),
        }
    }

    /// Returns the Stream ID
    fn stream_id(&self) -> StreamId {
        match self {
            RecvStream::Quic(stream) => stream.stream_id(),
            RecvStream::Tcp(stream) => stream.stream_id(),
        }
    }

    /// Receives data from the stream
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize> {
        match self {
            RecvStream::Quic(stream) => stream.receive_data(buffer).await,
            RecvStream::Tcp(stream) => stream.receive_data(buffer).await,
        }
    }

    /// Receive a frame over the stream
    async fn receive_frame(&mut self) -> Result<Frame> {
        match self {
            RecvStream::Quic(stream) => stream.receive_frame().await,
            RecvStream::Tcp(stream) => stream.receive_frame().await,
        }
    }

    /// Receive a file over the stream
    async fn receive_file(&mut self, file_path: &Path) -> Result<u64> {
        match self {
            RecvStream::Quic(stream) => stream.receive_file(file_path).await,
            RecvStream::Tcp(stream) => stream.receive_file(file_path).await,
        }
    }

    /// Gracefully closes the stream.
    async fn close(&mut self) -> Result<()> {
        match self {
            RecvStream::Quic(stream) => stream.close().await,
            RecvStream::Tcp(stream) => stream.close().await,
        }
    }

    /// Returns the current state of the connection.
    fn is_closed(&self) -> bool {
        match self {
            RecvStream::Quic(stream) => stream.is_closed(),
            RecvStream::Tcp(stream) => stream.is_closed(),
        }
    }

    /// Returns the remote address of the connection.
    fn remote_address(&self) -> SocketAddr {
        match self {
            RecvStream::Quic(stream) => stream.remote_address(),
            RecvStream::Tcp(stream) => stream.remote_address(),
        }
    }

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

    fn operation_id(&self) -> OperationId {
        match self {
            NetworkStream::Quic(stream) => stream.operation_id(),
            NetworkStream::Tcp(stream) => stream.operation_id(),
        }
    }

    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId> {
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

    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
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

    async fn send_file(&mut self, file_path: &Path) -> Result<OperationId> {
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

    fn split(self) -> (SendStream, RecvStream) {
        match self {
            NetworkStream::Quic(stream) => {
                let (send, recv) = stream.split();
                (send, recv)
            }
            NetworkStream::Tcp(stream) => {
                let (send, recv) = stream.split();
                (send, recv)
            }
        }
    }

    /// Merges a `SendStream` and a `RecvStream` back into a `NetworkStream`.
    fn merge(send_stream: SendStream, recv_stream: RecvStream) -> Result<Self> {
        match (send_stream, recv_stream) {
            // QUICのストリームをマージ
            (SendStream::Quic(send), RecvStream::Quic(recv)) => {
                Ok(NetworkStream::Quic(QuicStream::merge(SendStream::Quic(send), RecvStream::Quic(recv))?))
            }

            // TCPのストリームをマージ
            (SendStream::Tcp(send), RecvStream::Tcp(recv)) => {
                Ok(NetworkStream::Tcp(TlsTcpStream::merge(SendStream::Tcp(send), RecvStream::Tcp(recv))?))
            }

            // 不一致があればエラー
            _ => Err(anyhow::anyhow!("SendStream and RecvStream types do not match")),
        }
    }
}
