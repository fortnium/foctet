use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use foctet_core::frame::{ContentId, Frame, FrameHeader, FrameType, Payload};
use foctet_core::node::{ConnectionId, NodeId};
use futures::sink::SinkExt;
use tokio::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig, VarInt};
use anyhow::Result;
use anyhow::anyhow;
use crate::config::SocketConfig;
use super::{endpoint, ConnectionState, FoctetStream};

pub struct QuicStream {
    pub send_stream: SendStream,
    pub recv_stream: RecvStream,
    pub node_id: NodeId,
    pub stream_id: u64,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
}

impl FoctetStream for QuicStream {
    async fn send_data(&mut self, data: &[u8], content_id: Option<ContentId>) -> Result<()> {
        let mut framed_writer = FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut offset = 0;
        while offset < data.len() {
            let end = std::cmp::min(offset + self.send_buffer_size, data.len());
            let chunk = Payload::DataChunk(data[offset..end].to_vec());
            let message = Frame {
                header: FrameHeader {
                    frame_type: FrameType::DataTransfer,
                    node_id: self.node_id.clone(),
                    connection_id: Some(self.connection_id.clone()),
                    content_id: content_id.clone(),
                },
                payload: Some(chunk),
            };
            let serialized_message = message.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
            offset = end;
        }
        framed_writer.flush().await?;
        framed_writer.get_mut().finish()?;
        Ok(())
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>, content_id: Option<ContentId>) -> Result<usize> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut total_bytes_read: usize = 0;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(connection_id) = frame.header.connection_id {
                        if connection_id != self.connection_id {
                            continue;
                        }
                    }
                    if frame.header.content_id != content_id {
                        continue;
                    }
                    if let Some(Payload::DataChunk(data)) = frame.payload {
                        buffer.extend_from_slice(&data);
                        total_bytes_read += data.len();
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading from stream: {:?}", e);
                    break;
                }
            }
        }
        Ok(total_bytes_read)
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        let mut framed_writer = FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;
        framed_writer.flush().await?;
        framed_writer.get_mut().finish()?;
        Ok(())
    }

    async fn receive_frame(&mut self, content_id: Option<ContentId>) -> Result<Frame> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(connection_id) = &frame.header.connection_id {
                        if connection_id != &self.connection_id {
                            continue;
                        }
                    }
                    if frame.header.content_id != content_id {
                        continue;
                    }
                    return Ok(frame);
                }
                Err(e) => {
                    tracing::error!("Error reading from stream: {:?}", e);
                    break;
                }
            }
        }
        Ok(Frame::empty())
    }

    async fn send_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<()> {
        let mut framed_writer = FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = vec![0u8; self.send_buffer_size];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            let chunk = Payload::FileChunk(buffer[..n].to_vec());
            let message = Frame {
                header: FrameHeader {
                    frame_type: FrameType::FileTransfer,
                    node_id: self.node_id.clone(),
                    connection_id: Some(self.connection_id.clone()),
                    content_id: content_id.clone(),
                },
                payload: Some(chunk),
            };
            let serialized_message = message.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
        }
        framed_writer.flush().await?;
        framed_writer.get_mut().finish()?;
        Ok(())
    }

    async fn receive_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<u64> {
        let mut total_bytes: u64 = 0;
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::create(file_path).await?;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(connection_id) = &frame.header.connection_id {
                        if connection_id != &self.connection_id {
                            continue;
                        }
                    }
                    if frame.header.content_id != content_id {
                        continue;
                    }
                    if let Some(Payload::FileChunk(data)) = frame.payload {
                        file.write_all(&data).await?;
                        total_bytes += data.len() as u64;
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading from stream: {:?}", e);
                    break;
                }
            }
        }
        file.flush().await?;
        Ok(total_bytes)
    }

    async fn close(&mut self) -> Result<()> {
        self.send_stream.finish()?;
        self.recv_stream.stop(VarInt::from_u32(0))?;
        Ok(())
    }
}

pub struct QuicConnection {
    pub node_id: NodeId,
    pub connection_id: ConnectionId,
    /// The QUIC connection
    pub connection: Connection,
    /// The streams of the QUIC connection
    /// Key: Stream ID
    /// Value: The QUIC stream (SendStream, RecvStream)
    pub streams: HashMap<u64, Arc<QuicStream>>,
    pub state: ConnectionState,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    next_stream_id: u64,
}

impl QuicConnection {
    pub fn new(node_id: NodeId, connection: Connection, config: &SocketConfig) -> Self {
        Self {
            node_id: node_id,
            connection_id: ConnectionId::new(),
            connection,
            streams: HashMap::new(),
            state: ConnectionState::Connected,
            send_buffer_size: config.write_buffer_size(),
            receive_buffer_size: config.read_buffer_size(),
            next_stream_id: 0,
        }
    }

    pub async fn open_stream(&mut self) -> Result<Arc<QuicStream>> {
        let (send_stream, recv_stream) = self.connection.open_bi().await?;
        let quic_stream = Arc::new(QuicStream {
            send_stream: send_stream,
            recv_stream: recv_stream,
            node_id: self.node_id.clone(),
            stream_id: self.next_stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            receive_buffer_size: self.receive_buffer_size,
        });
        self.streams.insert(self.next_stream_id, Arc::clone(&quic_stream));
        tracing::info!("Opened bi-directional stream with ID: {}", self.next_stream_id);
        self.next_stream_id += 1;
        Ok(quic_stream)
    }

    pub async fn accept_stream(&mut self) -> Result<Arc<QuicStream>> {
        let (send_stream, recv_stream) = self.connection.accept_bi().await?;
        let quic_stream = Arc::new(QuicStream {
            send_stream: send_stream,
            recv_stream: recv_stream,
            node_id: self.node_id.clone(),
            stream_id: self.next_stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            receive_buffer_size: self.receive_buffer_size,
        });
        self.streams.insert(self.next_stream_id, Arc::clone(&quic_stream));
        tracing::info!("Accepted bi-directional stream with ID: {}", self.next_stream_id);
        self.next_stream_id += 1;
        Ok(quic_stream)
    }

    pub fn get_stream(&mut self, stream_id: u64) -> Option<&mut Arc<QuicStream>> {
        self.streams.get_mut(&stream_id)
    }

    pub async fn close_stream(&mut self, stream_id: u64) -> Result<()> {
        if let Some(stream) = self.streams.remove(&stream_id) {
            let mut stream = Arc::try_unwrap(stream).map_err(|_| anyhow!("Failed to close stream"))?;
            stream.close().await?;
        }
        Ok(())
    }
    pub async fn close(&mut self) -> Result<()> {
        // collect all stream IDs
        let stream_ids: Vec<u64> = self.streams.keys().cloned().collect();
        // close all streams
        for stream_id in stream_ids {
            self.close_stream(stream_id).await?;
        }
        // close the connection
        self.connection.close(0u32.into(), b"");
        Ok(())
    }
}

pub struct QuicSocket {
    pub node_id: NodeId,
    pub config: SocketConfig,
    pub endpoint: Endpoint,
    pub connections: HashMap<u64, Arc<QuicConnection>>,
    next_connection_id: u64,
}

impl QuicSocket {
    pub fn new(node_id: NodeId, config: SocketConfig) -> Result<Self> {
        let client_config: ClientConfig = endpoint::make_client_config(config.tls_config.client_config.clone())?;
        let server_config: ServerConfig = endpoint::make_server_config(config.tls_config.server_config.clone())?;
        let mut endpoint: Endpoint = Endpoint::server(server_config, config.server_addr)?;
        endpoint.set_default_client_config(client_config);
        Ok(Self {
            node_id: node_id,
            config: config,
            endpoint: endpoint,
            connections: HashMap::new(),
            next_connection_id: 0,
        })
    }

    pub async fn connect(&mut self, server_addr: SocketAddr, server_name: &str) -> Result<u64> {
        let connection = self.endpoint.connect(server_addr, server_name)?.await?;
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        
        self.connections.insert(id, Arc::new(QuicConnection::new(self.node_id.clone(), connection, &self.config)));
        Ok(id)
    }

    pub async fn listen(&mut self, sender: mpsc::Sender<Arc<QuicConnection>>) -> Result<()> {
        while let Some(incoming) = self.endpoint.accept().await {
            match incoming.await {
                Ok(connection) => {
                    let id = self.next_connection_id;
                    let quic_connection = Arc::new(QuicConnection::new(self.node_id.clone(), connection, &self.config));
                    let quic_connection_clone = Arc::clone(&quic_connection);
                    self.connections.insert(id, quic_connection);
                    self.next_connection_id += 1;
                    sender.send(quic_connection_clone).await?;
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {:?}", e);
                }
            };
        }
        Ok(())
    }

    pub fn get_connection(&self, id: u64) -> Option<Arc<QuicConnection>> {
        self.connections.get(&id).cloned()
    }

    pub fn get_all_connections(&self) -> Vec<Arc<QuicConnection>> {
        self.connections.values().cloned().collect()
    }

    pub async fn close_connection(&mut self, id: u64) -> Result<()> {
        if let Some(connection) = self.connections.remove(&id) {
            let mut connection = Arc::try_unwrap(connection).map_err(|_| anyhow!("Failed to remove connection"))?;
            connection.close().await?;
        }
        Ok(())
    }
    // TODO: add more methods
}
