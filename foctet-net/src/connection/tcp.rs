use super::FoctetStream;
use crate::socket::SocketConfig;
use anyhow::Result;
use foctet_core::error::StreamError;
use foctet_core::frame::{Frame, FrameType, Payload, StreamId};
use foctet_core::node::{ConnectionId, NodeId};
use futures::sink::SinkExt;
use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Debug)]
pub struct TlsTcpStream {
    pub stream: TlsStream<TcpStream>,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
    pub next_operation_id: u64,
    pub remote_address: SocketAddr,
}

impl FoctetStream for TlsTcpStream {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    async fn send_data(&mut self, data: &[u8]) -> Result<()> {
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
        self.next_operation_id += 1;
        Ok(())
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

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
        let serialized_message = frame.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        self.next_operation_id += 1;
        Ok(())
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

    async fn send_file(&mut self, file_path: &std::path::Path) -> Result<()> {
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
        self.next_operation_id += 1;
        Ok(())
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
}

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
            next_operation_id: 0,
            remote_address: remote_address,
        };
        Ok(tls_tcp_stream)
    }

    pub async fn listen(&mut self, sender: mpsc::Sender<TlsTcpStream>) -> Result<()> {
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
                next_operation_id: 0,
                remote_address: remote_address,
            };
            sender.send(tls_tcp_stream).await?;
        }
        Ok(())
    }
}
