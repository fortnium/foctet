use super::{FoctetRecvStream, FoctetSendStream};
use super::{endpoint, FoctetStream};
use crate::socket::SocketConfig;
use anyhow::anyhow;
use anyhow::Result;
use foctet_core::error::{ConnectionError, StreamError};
use foctet_core::frame::OperationId;
use foctet_core::frame::{Frame, FrameType, Payload, StreamId};
use foctet_core::node::NodeAddr;
use foctet_core::node::{ConnectionId, NodeId};
use foctet_core::state::ConnectionState;
use futures::sink::SinkExt;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig, VarInt};
use tokio_util::sync::CancellationToken;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Debug)]
pub struct QuicSendStream {
    pub send_stream: SendStream,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub is_closed: bool,
    pub next_operation_id: OperationId,
    pub remote_address: SocketAddr,
}

impl FoctetSendStream for QuicSendStream {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    fn operation_id(&self) -> OperationId {
        self.next_operation_id
    }
    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut offset = 0;
        while offset < data.len() {
            let end = std::cmp::min(offset + self.send_buffer_size, data.len());
            let chunk = Payload::DataChunk(data[offset..end].to_vec());
            // Check if this is the last chunk
            let is_last_frame = end == data.len();
            let frame: Frame = Frame::builder()
                .with_fin(is_last_frame)
                .with_frame_type(FrameType::DataTransfer)
                .with_operation_id(self.next_operation_id)
                .with_payload(chunk)
                .build();
            let serialized_message = frame.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;

            offset = end;
        }
        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }
    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }
    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = vec![0u8; self.send_buffer_size];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            let chunk = Payload::FileChunk(buffer[..n].to_vec());
            let frame: Frame = Frame::builder()
                .with_fin(false)
                .with_frame_type(FrameType::FileTransfer)
                .with_operation_id(self.next_operation_id)
                .with_payload(chunk)
                .build();
            let serialized_message = frame.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
        }

        // Send the last frame with the FIN flag and NO payload
        let frame: Frame = Frame::builder()
            .with_fin(true)
            .with_frame_type(FrameType::FileTransfer)
            .with_operation_id(self.next_operation_id)
            .build();
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn close(&mut self) -> Result<()> {
        self.send_stream.finish()?;
        self.is_closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed
    }

    fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }
}

#[derive(Debug)]
pub struct QuicRecvStream {
    pub recv_stream: RecvStream,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
    pub remote_address: SocketAddr,
}

impl FoctetRecvStream for QuicRecvStream {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut total_bytes_read: usize = 0;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(Payload::DataChunk(data)) = frame.payload {
                        buffer.extend_from_slice(&data);
                        total_bytes_read += data.len();
                    }
                    if frame.fin {
                        break;
                    }
                }
                Err(e) => {
                    //Safely cast the error to a quinn::ReadError
                    match e.downcast::<quinn::ReadError>() {
                        Ok(read_err) => match read_err {
                            quinn::ReadError::ClosedStream => {
                                tracing::info!("Stream closed by peer");
                            }
                            quinn::ReadError::ConnectionLost(_) => {
                                tracing::info!("Connection closed by peer");
                            }
                            _ => {
                                tracing::error!("Error reading from stream: {:?}", read_err);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Error reading from stream: {:?}", e);
                        }
                    }
                    break;
                }
            }
        }
        Ok(total_bytes_read)
    }
    async fn receive_frame(&mut self) -> Result<Frame> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    return Ok(frame);
                }
                Err(e) => {
                    //Safely cast the error to a quinn::ReadError
                    match e.downcast::<quinn::ReadError>() {
                        Ok(read_err) => match read_err {
                            quinn::ReadError::ClosedStream => {
                                tracing::info!("Stream closed by peer");
                            }
                            quinn::ReadError::ConnectionLost(_) => {
                                tracing::info!("Connection closed by peer");
                            }
                            _ => {
                                tracing::error!("Error reading from stream: {:?}", read_err);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Error reading from stream: {:?}", e);
                        }
                    }
                    break;
                }
            }
        }
        Err(StreamError::Closed.into())
    }
    async fn receive_file(&mut self, file_path: &std::path::Path) -> Result<u64> {
        let mut total_bytes: u64 = 0;
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::create(file_path).await?;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(Payload::FileChunk(data)) = frame.payload {
                        file.write_all(&data).await?;
                        total_bytes += data.len() as u64;
                    }
                    if frame.fin {
                        break;
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
        self.recv_stream.stop(VarInt::from_u32(0))?;
        self.is_closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed
    }

    fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }
}

#[derive(Debug)]
pub struct QuicStream {
    pub send_stream: SendStream,
    pub recv_stream: RecvStream,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
    pub next_operation_id: OperationId,
    pub remote_address: SocketAddr,
}

impl FoctetStream for QuicStream {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    fn operation_id(&self) -> OperationId {
        self.next_operation_id
    }
    async fn send_data(&mut self, data: &[u8]) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut offset = 0;
        while offset < data.len() {
            let end = std::cmp::min(offset + self.send_buffer_size, data.len());
            let chunk = Payload::DataChunk(data[offset..end].to_vec());
            // Check if this is the last chunk
            let is_last_frame = end == data.len();
            let frame: Frame = Frame::builder()
                .with_fin(is_last_frame)
                .with_frame_type(FrameType::DataTransfer)
                .with_operation_id(self.next_operation_id)
                .with_payload(chunk)
                .build();
            let serialized_message = frame.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;

            offset = end;
        }
        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut total_bytes_read: usize = 0;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(Payload::DataChunk(data)) = frame.payload {
                        buffer.extend_from_slice(&data);
                        total_bytes_read += data.len();
                    }
                    if frame.fin {
                        break;
                    }
                }
                Err(e) => {
                    //Safely cast the error to a quinn::ReadError
                    match e.downcast::<quinn::ReadError>() {
                        Ok(read_err) => match read_err {
                            quinn::ReadError::ClosedStream => {
                                tracing::info!("Stream closed by peer");
                            }
                            quinn::ReadError::ConnectionLost(_) => {
                                tracing::info!("Connection closed by peer");
                            }
                            _ => {
                                tracing::error!("Error reading from stream: {:?}", read_err);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Error reading from stream: {:?}", e);
                        }
                    }
                    break;
                }
            }
        }
        Ok(total_bytes_read)
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    return Ok(frame);
                }
                Err(e) => {
                    //Safely cast the error to a quinn::ReadError
                    match e.downcast::<quinn::ReadError>() {
                        Ok(read_err) => match read_err {
                            quinn::ReadError::ClosedStream => {
                                tracing::info!("Stream closed by peer");
                            }
                            quinn::ReadError::ConnectionLost(_) => {
                                tracing::info!("Connection closed by peer");
                            }
                            _ => {
                                tracing::error!("Error reading from stream: {:?}", read_err);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Error reading from stream: {:?}", e);
                        }
                    }
                    break;
                }
            }
        }
        Err(StreamError::Closed.into())
    }

    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<OperationId> {
        let mut framed_writer =
            FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = vec![0u8; self.send_buffer_size];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            let chunk = Payload::FileChunk(buffer[..n].to_vec());
            let frame: Frame = Frame::builder()
                .with_fin(false)
                .with_frame_type(FrameType::FileTransfer)
                .with_operation_id(self.next_operation_id)
                .with_payload(chunk)
                .build();
            let serialized_message = frame.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
        }

        // Send the last frame with the FIN flag and NO payload
        let frame: Frame = Frame::builder()
            .with_fin(true)
            .with_frame_type(FrameType::FileTransfer)
            .with_operation_id(self.next_operation_id)
            .build();
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().finish()?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_file(&mut self, file_path: &std::path::Path) -> Result<u64> {
        let mut total_bytes: u64 = 0;
        let mut framed_reader = FramedRead::new(&mut self.recv_stream, LengthDelimitedCodec::new());
        let mut file = tokio::fs::File::create(file_path).await?;
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    if let Some(Payload::FileChunk(data)) = frame.payload {
                        file.write_all(&data).await?;
                        total_bytes += data.len() as u64;
                    }
                    if frame.fin {
                        break;
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
        self.is_closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed
    }

    fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    fn split(self) -> (super::SendStream, super::RecvStream) {
        let quic_send_stream = QuicSendStream {
            send_stream: self.send_stream,
            node_id: self.node_id.clone(),
            stream_id: self.stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            is_closed: self.is_closed,
            next_operation_id: self.next_operation_id,
            remote_address: self.remote_address,
        };
        let quic_recv_stream = QuicRecvStream {
            recv_stream: self.recv_stream,
            node_id: self.node_id.clone(),
            stream_id: self.stream_id,
            connection_id: self.connection_id.clone(),
            receive_buffer_size: self.receive_buffer_size,
            is_closed: self.is_closed,
            remote_address: self.remote_address,
        };
        let send_stream = super::SendStream::Quic(quic_send_stream);
        let recv_stream = super::RecvStream::Quic(quic_recv_stream);
        (send_stream, recv_stream)
    }
}

#[derive(Debug)]
pub struct QuicConnection {
    pub node_id: NodeId,
    pub connection_id: ConnectionId,
    /// The QUIC connection
    pub connection: Connection,
    pub state: ConnectionState,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub next_stream_id: StreamId,
}

impl QuicConnection {
    pub fn new(node_id: NodeId, connection: Connection, config: &SocketConfig) -> Self {
        Self {
            node_id: node_id,
            connection_id: ConnectionId::new(),
            connection: connection,
            state: ConnectionState::Connected,
            send_buffer_size: config.write_buffer_size(),
            receive_buffer_size: config.read_buffer_size(),
            next_stream_id: StreamId::new(0),
        }
    }

    pub async fn open_stream(&mut self) -> Result<QuicStream> {
        let (send_stream, recv_stream) = self.connection.open_bi().await?;
        let quic_stream = QuicStream {
            send_stream: send_stream,
            recv_stream: recv_stream,
            node_id: self.node_id.clone(),
            stream_id: self.next_stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            receive_buffer_size: self.receive_buffer_size,
            is_closed: false,
            next_operation_id: OperationId(0),
            remote_address: self.remote_address(),
        };
        tracing::info!(
            "Opened bi-directional stream with ID: {}",
            self.next_stream_id
        );
        self.next_stream_id.increment();
        Ok(quic_stream)
    }

    pub async fn accept_stream(&mut self) -> Result<QuicStream> {
        let (send_stream, recv_stream) = match self.connection.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => match e {
                quinn::ConnectionError::ApplicationClosed(_) => {
                    self.state = ConnectionState::Disconnected;
                    return Err(ConnectionError::Closed.into());
                }
                quinn::ConnectionError::ConnectionClosed(_) => {
                    self.state = ConnectionState::Disconnected;
                    return Err(ConnectionError::Closed.into());
                }
                _ => {
                    return Err(anyhow!("Error accepting stream"));
                }
            },
        };
        let quic_stream = QuicStream {
            send_stream: send_stream,
            recv_stream: recv_stream,
            node_id: self.node_id.clone(),
            stream_id: self.next_stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            receive_buffer_size: self.receive_buffer_size,
            is_closed: false,
            next_operation_id: OperationId(0),
            remote_address: self.remote_address(),
        };
        tracing::info!(
            "Accepted bi-directional stream with ID: {}",
            self.next_stream_id
        );
        self.next_stream_id.increment();
        Ok(quic_stream)
    }

    /// Close the QUIC connection
    pub async fn close(&mut self) -> Result<()> {
        // close the connection
        self.connection.close(0u32.into(), b"");
        self.state = ConnectionState::Disconnected;
        Ok(())
    }
    pub fn id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }
}

#[derive(Clone)]
pub struct QuicSocket {
    pub node_id: NodeId,
    pub config: SocketConfig,
    pub endpoint: Endpoint,
}

impl QuicSocket {
    /// Creates a new QUIC socket with given node_id and config.
    /// The socket acts as both a client and a server.
    pub fn new(node_id: NodeId, config: SocketConfig) -> Result<Self> {
        let client_config: ClientConfig =
            endpoint::make_client_config(config.tls_config.client_config.clone())?;
        let server_config: ServerConfig =
            endpoint::make_server_config(config.tls_config.server_config.clone())?;
        let mut endpoint: Endpoint = Endpoint::server(server_config, config.server_addr)?;
        endpoint.set_default_client_config(client_config);
        Ok(Self {
            node_id: node_id,
            config: config,
            endpoint: endpoint,
        })
    }
    /// Creates a new QUIC client with given node_id and config.
    /// The socket acts as a client.
    pub fn new_client(node_id: NodeId, config: SocketConfig) -> Result<Self> {
        let client_config = endpoint::make_client_config(config.tls_config.client_config.clone())?;
        let mut endpoint = Endpoint::client(config.bind_addr)?;
        endpoint.set_default_client_config(client_config);
        Ok(Self {
            node_id: node_id,
            config: config,
            endpoint: endpoint,
        })
    }
    pub async fn connect(
        &mut self,
        server_addr: SocketAddr,
        server_name: &str,
    ) -> Result<QuicConnection> {
        let connection = self.endpoint.connect(server_addr, server_name)?.await?;
        let quic_connection = QuicConnection::new(self.node_id.clone(), connection, &self.config);
        Ok(quic_connection)
    }
    /* pub async fn listen(&mut self, sender: mpsc::Sender<QuicConnection>) -> Result<()> {
        while let Some(incoming) = self.endpoint.accept().await {
            match incoming.await {
                Ok(connection) => {
                    let quic_connection =
                        QuicConnection::new(self.node_id.clone(), connection, &self.config);
                    sender.send(quic_connection).await?;
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {:?}", e);
                }
            };
        }
        Ok(())
    } */
    pub async fn listen(&mut self, sender: mpsc::Sender<QuicConnection>, cancel_token: CancellationToken) -> Result<()> {
        loop {
            tokio::select! {
                // Monitor the cancellation token
                _ = cancel_token.cancelled() => {
                    tracing::info!("QuicSocket listen cancelled");
                    break;
                }
                // Accept incoming connections
                incoming = self.endpoint.accept() => {
                    match incoming {
                        Some(incoming_connection) => {
                            match incoming_connection.await {
                                Ok(connection) => {
                                    let quic_connection = QuicConnection::new(self.node_id.clone(), connection, &self.config);
                                    if sender.send(quic_connection).await.is_err() {
                                        tracing::warn!("Failed to send QuicConnection to the channel");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Error accepting connection: {:?}", e);
                                }
                            }
                        }
                        None => {
                            tracing::warn!("No incoming connection; endpoint may have been closed");
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn connect_node(&mut self, node_addr: NodeAddr) -> Result<QuicConnection> {
        let sorted_addrs = super::priority::sort_socket_addrs(&node_addr.socket_addresses);
        let addrs = super::filter::filter_reachable_addrs(sorted_addrs, self.config.include_loopback);
        let server_name = node_addr.get_server_name();
        for addr in addrs {
            match self.connect(addr, &server_name).await {
                Ok(connection) => {
                    tracing::info!("Connected to {}", addr);
                    return Ok(connection);
                }
                Err(e) => {
                    tracing::error!("Error connecting to {}: {:?}", addr, e);
                }
            }
        }
        Err(anyhow!("Failed to connect to node"))
    }
}
