use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use foctet_core::error::StreamError;
use foctet_core::frame::{ContentId, Frame, FrameHeader, FrameType, Payload, StreamId};
use foctet_core::node::{ConnectionId, NodeId};
use foctet_core::state::ConnectionState;
use futures::sink::SinkExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use anyhow::Result;
use crate::config::SocketConfig;
use anyhow::anyhow;

use super::NetworkStream;

pub struct TlsTcpStream {
    pub stream: TlsStream<TcpStream>,
    pub node_id: NodeId,
    pub stream_id: StreamId,
    pub connection_id: ConnectionId,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub is_closed: bool,
}

impl NetworkStream for TlsTcpStream {
    async fn send_data(&mut self, data: &[u8], content_id: Option<ContentId>) -> Result<()> {
        let mut framed_writer: FramedWrite<&mut TlsStream<TcpStream>, LengthDelimitedCodec> = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
        let mut offset = 0;
        while offset < data.len() {
            let end = std::cmp::min(offset + self.send_buffer_size, data.len());
            let chunk = Payload::DataChunk(data[offset..end].to_vec());
            let message = Frame {
                header: FrameHeader {
                    frame_type: FrameType::DataTransfer,
                    node_id: self.node_id.clone(),
                    stream_id: self.stream_id.clone(),
                    connection_id: Some(self.connection_id.clone()),
                    content_id: content_id.clone(),
                },
                payload: Some(chunk),
            };
            let serialized_message = message.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
            offset = end;
        }
        // Send the end of transfer message
        let end_message = Frame::end_of_transfer(self.node_id.clone(), self.stream_id.clone(), Some(self.connection_id.clone()), content_id.clone());
        let serialized_message = end_message.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        Ok(())
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>, content_id: Option<ContentId>) -> Result<usize> {
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
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
                    match &frame.header.frame_type {
                        FrameType::DataTransfer => {
                            if let Some(Payload::DataChunk(data)) = frame.payload {
                                buffer.extend_from_slice(&data);
                                total_bytes_read += data.len();
                            }
                        },
                        FrameType::EndOfTransfer => {
                            tracing::info!("[{}]End of transfer detected", self.stream_id);
                            break;
                        },
                        _ => continue,
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

        // Send the end of transfer message
        let end_message = Frame::end_of_transfer(self.node_id.clone(), self.stream_id.clone(), Some(self.connection_id.clone()), frame.header.content_id);
        let serialized_message = end_message.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;

        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        Ok(())
    }

    async fn receive_frame(&mut self, content_id: Option<ContentId>) -> Result<Frame> {
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
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
        Err(StreamError::Closed.into())
    }

    async fn send_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<()> {
        let mut framed_writer = FramedWrite::new(&mut self.stream, LengthDelimitedCodec::new());
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
                    stream_id: self.stream_id.clone(),
                    connection_id: Some(self.connection_id.clone()),
                    content_id: content_id.clone(),
                },
                payload: Some(chunk),
            };
            let serialized_message = message.to_bytes()?;
            framed_writer.send(serialized_message.into()).await?;
        }
        // Send the end of transfer message
        let end_message = Frame::end_of_transfer(self.node_id.clone(), self.stream_id.clone(), Some(self.connection_id.clone()), content_id);
        let serialized_message = end_message.to_bytes()?;
        framed_writer.send(serialized_message.into()).await?;
        
        framed_writer.flush().await?;
        //framed_writer.get_mut().shutdown().await?;
        Ok(())
    }

    async fn receive_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<u64> {
        let mut total_bytes: u64 = 0;
        let mut framed_reader = FramedRead::new(&mut self.stream, LengthDelimitedCodec::new());
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
        self.stream.get_mut().0.shutdown().await?;
        self.is_closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed
    }
}

pub struct TcpConnection {
    pub node_id: NodeId,
    pub connection_id: ConnectionId,
    /// The TLS over TCP stream as the connection
    pub stream: Arc<Mutex<TlsTcpStream>>,
    pub state: ConnectionState,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
}

impl TcpConnection {
    pub fn new(node_id: NodeId, connection: TlsStream<TcpStream>, config: &SocketConfig) -> Self {
        let connection_id = ConnectionId::new();
        let tls_tcp_stream = TlsTcpStream {
            stream: connection,
            node_id: node_id.clone(),
            stream_id: StreamId::new(0),
            connection_id: connection_id.clone(),
            send_buffer_size: config.write_buffer_size(),
            receive_buffer_size: config.read_buffer_size(),
            is_closed: false,
        };
        Self {
            node_id: node_id,
            connection_id: connection_id,
            stream: Arc::new(Mutex::new(tls_tcp_stream)),
            state: ConnectionState::Connected,
            send_buffer_size: config.write_buffer_size(),
            receive_buffer_size: config.read_buffer_size(),
        }
    }
    pub async fn get_stream(&mut self) -> Arc<Mutex<TlsTcpStream>> {
        Arc::clone(&self.stream)
    }
    pub async fn close(&mut self) -> Result<()> {
        let mut connection = self.stream.lock().await;
        if connection.is_closed {
            Ok(())
        } else {
            connection.close().await
        }
    }
    pub fn id(&self) -> ConnectionId {
        self.connection_id.clone()
    }
}

pub struct TcpSocket {
    pub node_id: NodeId,
    pub config: SocketConfig,
    pub tls_connector: TlsConnector,
    pub tls_acceptor: TlsAcceptor,
    pub connections: Arc<Mutex<HashMap<ConnectionId, Arc<Mutex<TcpConnection>>>>>,
}

impl TcpSocket {
    pub fn new(node_id: NodeId, config: SocketConfig) -> Self {
        let tls_connector = TlsConnector::from(Arc::new(config.tls_config.client_config.clone()));
        let tls_acceptor = TlsAcceptor::from(Arc::new(config.tls_config.server_config.clone()));
        Self {
            node_id: node_id,
            config: config,
            tls_connector: tls_connector,
            tls_acceptor: tls_acceptor,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn connect(&mut self, server_addr: SocketAddr, server_name: &str) -> Result<Arc<Mutex<TcpConnection>>> {
        let name = rustls_pki_types::ServerName::try_from(server_name.to_string())?;
        let stream = TcpStream::connect(server_addr).await?;
        let tls_stream = self.tls_connector.connect(name, stream).await?;
        let tcp_connection = TcpConnection::new(self.node_id.clone(), TlsStream::Client(tls_stream), &self.config);
        let id = tcp_connection.id();
        let conn: Arc<Mutex<TcpConnection>> = Arc::new(Mutex::new(tcp_connection));        
        let mut connections = self.connections.lock().await;
        connections.insert(id.clone(), Arc::clone(&conn));
        Ok(conn)
    }

    pub async fn listen(&mut self, sender: mpsc::Sender<Arc<Mutex<TcpConnection>>>) -> Result<()> {
        let listener = TcpListener::bind(self.config.server_addr).await?;
        while let Ok((stream, _addr)) = listener.accept().await {
            let tls_stream = self.tls_acceptor.accept(stream).await?;
            let tcp_connection = TcpConnection::new(self.node_id.clone(), TlsStream::Server(tls_stream), &self.config);
            let id = tcp_connection.id();
            let conn: Arc<Mutex<TcpConnection>> = Arc::new(Mutex::new(tcp_connection));
            let mut connections = self.connections.lock().await;
            connections.insert(id, Arc::clone(&conn));
            sender.send(conn).await?;
        }
        Ok(())
    }

    pub async fn get_connection(&self, id: ConnectionId) -> Option<Arc<Mutex<TcpConnection>>> {
        let connections = self.connections.lock().await;
        connections.get(&id).cloned()
    }

    pub async fn get_all_connections(&self) -> Vec<Arc<Mutex<TcpConnection>>> {
        let connections = self.connections.lock().await;
        connections.values().cloned().collect()
    }

    pub async fn close_connection(&mut self, id: ConnectionId) -> Result<()> {
        // Lock the connections and take out the connection
        let connection = {
            let mut connections = self.connections.lock().await;
            connections.remove(&id)
        };
        // Close the connection if it exists
        if let Some(conn) = connection {
            let conn = Arc::try_unwrap(conn).map_err(|_| anyhow!("Failed to remove connection"))?;
            let mut conn = conn.lock().await;
            conn.close().await?;
        }
        Ok(())
    }

}
