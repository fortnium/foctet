use super::FoctetRecvStream;
use super::FoctetSendStream;
use super::FoctetStream;
use crate::socket::SocketConfig;
use anyhow::anyhow;
use anyhow::Result;
use foctet_core::error::StreamError;
use foctet_core::frame::OperationId;
use foctet_core::frame::{Frame, FrameType, Payload, StreamId};
use foctet_core::node::{ConnectionId, NodeAddr, NodeId};
use futures::sink::SinkExt;
use tokio_util::sync::CancellationToken;
use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio::io::{ReadHalf, WriteHalf};

#[derive(Debug)]
pub struct TlsTcpSendStream {
    pub send_stream: WriteHalf<TlsStream<TcpStream>>,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub is_closed: bool,
    pub next_operation_id: OperationId,
    pub remote_address: SocketAddr,
}

impl FoctetSendStream for TlsTcpSendStream {
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
        let mut framed_writer: FramedWrite<&mut WriteHalf<TlsStream<TcpStream>>, LengthDelimitedCodec> =
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
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }
    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
        let mut framed_writer = FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }
    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<OperationId> {
        let mut framed_writer = FramedWrite::new(&mut self.send_stream, LengthDelimitedCodec::new());
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
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }
    async fn close(&mut self) -> Result<()> {
        self.send_stream.shutdown().await?;
        //self.stream.get_mut().0.shutdown().await?;
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
pub struct TlsTcpRecvStream {
    pub recv_stream: ReadHalf<TlsStream<TcpStream>>,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
    pub remote_address: SocketAddr,
}

impl FoctetRecvStream for TlsTcpRecvStream {
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
                    tracing::error!("Error reading from stream: {:?}", e);
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
                    tracing::error!("Error reading from stream: {:?}", e);
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
        //self.recv_stream.shutdown().await?;
        //self.stream.get_mut().0.shutdown().await?;
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
pub struct TlsTcpStream {
    pub stream: TlsStream<TcpStream>,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
    pub next_operation_id: OperationId,
    pub remote_address: SocketAddr,
}

impl FoctetStream for TlsTcpStream {
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
        let mut framed_writer: FramedWrite<&mut TlsStream<TcpStream>, LengthDelimitedCodec> =
            FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
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
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>) -> Result<usize> {
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
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
                    tracing::error!("Error reading from stream: {:?}", e);
                    break;
                }
            }
        }
        Ok(total_bytes_read)
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<OperationId> {
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_frame(&mut self) -> Result<Frame> {
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
        while let Some(chunk) = framed_reader.next().await {
            match chunk {
                Ok(bytes) => {
                    let frame = Frame::from_bytes(&bytes)?;
                    return Ok(frame);
                }
                Err(e) => {
                    tracing::error!("Error reading from stream: {:?}", e);
                    break;
                }
            }
        }
        Err(StreamError::Closed.into())
    }

    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<OperationId> {
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
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
        //framed_writer.get_mut().shutdown().await?;
        let operation_id = self.operation_id();
        self.next_operation_id.increment();
        Ok(operation_id)
    }

    async fn receive_file(&mut self, file_path: &std::path::Path) -> Result<u64> {
        let mut total_bytes: u64 = 0;
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
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
        self.stream.shutdown().await?;
        //self.stream.get_mut().0.shutdown().await?;
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
        let (read_half, write_half) = tokio::io::split(self.stream);
        let tcp_send_stream = TlsTcpSendStream {
            send_stream: write_half,
            node_id: self.node_id.clone(),
            stream_id: self.stream_id,
            connection_id: self.connection_id.clone(),
            send_buffer_size: self.send_buffer_size,
            is_closed: self.is_closed,
            next_operation_id: self.next_operation_id,
            remote_address: self.remote_address,
        };
        let tcp_recv_stream = TlsTcpRecvStream {
            recv_stream: read_half,
            node_id: self.node_id.clone(),
            stream_id: self.stream_id,
            connection_id: self.connection_id,
            receive_buffer_size: self.receive_buffer_size,
            is_closed: self.is_closed,
            remote_address: self.remote_address,
        };
        let send_stream = super::SendStream::Tcp(tcp_send_stream);
        let recv_stream = super::RecvStream::Tcp(tcp_recv_stream);
        (send_stream, recv_stream)
    }
    fn merge(send_stream: super::SendStream, recv_stream: super::RecvStream) -> Result<Self> where Self: Sized {
        match (send_stream, recv_stream) {
            (super::SendStream::Tcp(tcp_send_stream), super::RecvStream::Tcp(tcp_recv_stream)) => {
                let stream: TlsStream<TcpStream> = tcp_recv_stream.recv_stream.unsplit(tcp_send_stream.send_stream);
                Ok(Self {
                    stream: stream,
                    node_id: tcp_send_stream.node_id,
                    stream_id: tcp_send_stream.stream_id,
                    connection_id: tcp_send_stream.connection_id,
                    send_buffer_size: tcp_send_stream.send_buffer_size,
                    receive_buffer_size: tcp_recv_stream.receive_buffer_size,
                    is_closed: tcp_send_stream.is_closed,
                    next_operation_id: tcp_send_stream.next_operation_id,
                    remote_address: tcp_send_stream.remote_address,
                })
            }
            _ => Err(anyhow!("Invalid stream types")),
        }
    }
}

#[derive(Clone)]
pub struct TcpSocket {
    pub node_id: NodeId,
    pub config: SocketConfig,
    pub tls_connector: TlsConnector,
    pub tls_acceptor: TlsAcceptor,
}

impl TcpSocket {
    /// Creates a new TCP socket with given node_id and config
    /// The socket acts as both a client and a server.
    pub fn new(node_id: NodeId, config: SocketConfig) -> Result<Self> {
        let tls_connector = TlsConnector::from(Arc::new(config.tls_config.client_config.clone()));
        let tls_acceptor = TlsAcceptor::from(Arc::new(config.tls_config.server_config.clone()));
        Ok(Self {
            node_id: node_id,
            config: config,
            tls_connector: tls_connector,
            tls_acceptor: tls_acceptor,
        })
    }

    pub async fn connect(
        &mut self,
        server_addr: SocketAddr,
        server_name: &str,
    ) -> Result<TlsTcpStream> {
        let name = rustls_pki_types::ServerName::try_from(server_name.to_string())?;
        let stream = TcpStream::connect(server_addr).await?;
        let remote_address = stream.peer_addr()?;
        let tls_stream = self.tls_connector.connect(name, stream).await?;
        let tls_tcp_stream = TlsTcpStream {
            stream: TlsStream::Client(tls_stream),
            node_id: self.node_id.clone(),
            stream_id: StreamId::new(0),
            connection_id: ConnectionId::new(),
            send_buffer_size: self.config.write_buffer_size(),
            receive_buffer_size: self.config.read_buffer_size(),
            is_closed: false,
            next_operation_id: OperationId(0),
            remote_address: remote_address,
        };
        Ok(tls_tcp_stream)
    }

    /* pub async fn listen(&mut self, sender: mpsc::Sender<TlsTcpStream>) -> Result<()> {
        let listener = TcpListener::bind(self.config.server_addr).await?;
        while let Ok((stream, _addr)) = listener.accept().await {
            let remote_address = stream.peer_addr()?;
            let tls_stream = self.tls_acceptor.accept(stream).await?;
            let tls_tcp_stream = TlsTcpStream {
                stream: TlsStream::Server(tls_stream),
                node_id: self.node_id.clone(),
                stream_id: StreamId::new(0),
                connection_id: ConnectionId::new(),
                send_buffer_size: self.config.write_buffer_size(),
                receive_buffer_size: self.config.read_buffer_size(),
                is_closed: false,
                next_operation_id: OperationId(0),
                remote_address: remote_address,
            };
            sender.send(tls_tcp_stream).await?;
        }
        Ok(())
    } */

    pub async fn listen(&mut self, sender: mpsc::Sender<TlsTcpStream>, cancel_token: CancellationToken) -> Result<()> {
        let listener = TcpListener::bind(self.config.server_addr).await?;
        tracing::info!("Listening on {}", self.config.server_addr);
    
        loop {
            tokio::select! {
                // Monitor the cancellation token
                _ = cancel_token.cancelled() => {
                    tracing::info!("TcpSocket listen cancelled");
                    break;
                }
    
                // Accept incoming connections
                incoming = listener.accept() => {
                    match incoming {
                        Ok((stream, addr)) => {
                            let remote_address = stream.peer_addr()?;
                            tracing::info!("Accepted connection from {}", addr);
                            match self.tls_acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    let tls_tcp_stream = TlsTcpStream {
                                        stream: TlsStream::Server(tls_stream),
                                        node_id: self.node_id.clone(),
                                        stream_id: StreamId::new(0),
                                        connection_id: ConnectionId::new(),
                                        send_buffer_size: self.config.write_buffer_size(),
                                        receive_buffer_size: self.config.read_buffer_size(),
                                        is_closed: false,
                                        next_operation_id: OperationId(0),
                                        remote_address,
                                    };
                                    if sender.send(tls_tcp_stream).await.is_err() {
                                        tracing::warn!("Failed to send TlsTcpStream to the channel");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to complete TLS handshake: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error accepting TCP connection: {:?}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn connect_node(&mut self, node_addr: NodeAddr) -> Result<TlsTcpStream> {
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
