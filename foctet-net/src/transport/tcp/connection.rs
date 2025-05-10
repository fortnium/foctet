use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use super::stream::LogicalTcpStream;
use crate::transport::connection::ConnectionHandle;
use crate::transport::stream::Stream;
use anyhow::anyhow;
use anyhow::Result;
use foctet_core::connection::Direction;
use foctet_core::id::NodeId;
use foctet_core::default;
use foctet_mux::Session;
use stackaddr::StackAddr;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;

pub struct TcpConnection {
    direction: Direction,
    local_addr: StackAddr,
    remote_addr: StackAddr,
    local_node_id: NodeId,
    remote_node_id: NodeId,
    connection: Session<TlsStream<TcpStream>>,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
    next_stream_id: AtomicU32,
}

impl TcpConnection {
    pub fn new(
        direction: Direction,
        local_addr: StackAddr,
        remote_addr: StackAddr,
        connection: Session<TlsStream<TcpStream>>,
    ) -> Self {
        Self {
            direction,
            local_addr,
            remote_addr,
            local_node_id: NodeId::zero(),
            remote_node_id: NodeId::zero(),
            connection,
            write_buffer_size: default::DEFAULT_WRITE_BUFFER_SIZE,
            read_buffer_size: default::DEFAULT_READ_BUFFER_SIZE,
            next_stream_id: AtomicU32::new(1),
        }
    }
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }
    pub fn with_local_addr(mut self, local_addr: StackAddr) -> Self {
        self.local_addr = local_addr;
        self
    }
    pub fn with_remote_addr(mut self, remote_addr: StackAddr) -> Self {
        self.remote_addr = remote_addr;
        self
    }
    pub fn with_local_node_id(
        mut self,
        local_node_id: NodeId,
    ) -> Self {
        self.local_node_id = local_node_id;
        self
    }
    pub fn with_remote_node_id(
        mut self,
        remote_node_id: NodeId,
    ) -> Self {
        self.remote_node_id = remote_node_id;
        self
    }
    pub fn with_connection(mut self, connection: Session<TlsStream<TcpStream>>) -> Self {
        self.connection = connection;
        self
    }
    pub fn with_write_buffer_size(mut self, write_buffer_size: usize) -> Self {
        self.write_buffer_size = write_buffer_size;
        self
    }
    pub fn with_read_buffer_size(mut self, read_buffer_size: usize) -> Self {
        self.read_buffer_size = read_buffer_size;
        self
    }
    pub fn with_initial_stream_id(mut self, ini_stream_id: AtomicU32) -> Self {
        self.next_stream_id = ini_stream_id;
        self
    }
}

impl ConnectionHandle for TcpConnection {
    fn direction(&self) -> Direction {
        self.direction
    }

    fn local_addr(&self) -> StackAddr {
        self.local_addr.clone()
    }

    fn remote_addr(&self) -> StackAddr {
        self.remote_addr.clone()
    }

    fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }

    fn remote_node_id(&self) -> NodeId {
        self.remote_node_id
    }

    async fn open_stream(&mut self) -> Result<Stream> {
        let logical_stream = self.connection.open_stream().await?;
        self.next_stream_id.fetch_add(1, Ordering::SeqCst);
        let stream_id = logical_stream.stream_id();
        tracing::info!("Opened logical stream: {:?}", logical_stream);
        let (writer, reader) = logical_stream.split();
        let logical_tcp_stream = LogicalTcpStream::new(
            stream_id,
            writer,
            reader,
            self.write_buffer_size,
            self.read_buffer_size,
        );
        Ok(Stream::Tcp(logical_tcp_stream))
    }

    async fn accept_stream(&mut self) -> Result<Stream> {
        tracing::info!("Waiting for logical stream...");
        let logical_stream = match self.connection.accept_stream().await {
            Ok(stream) => stream,
            Err(e) => return Err(anyhow!("Error accepting stream: {:?}", e)),
        };
        self.next_stream_id.fetch_add(1, Ordering::SeqCst);
        let stream_id = logical_stream.stream_id();
        tracing::info!("Accepted logical stream: {:?}", logical_stream);
        let (writer, reader) = logical_stream.split();
        let logical_tcp_stream = LogicalTcpStream::new(
            stream_id,
            writer,
            reader,
            self.write_buffer_size,
            self.read_buffer_size,
        );
        Ok(Stream::Tcp(logical_tcp_stream))
    }

    async fn close(&mut self) -> Result<()> {
        let _ = self.connection.shutdown().await;
        Ok(())
    }
}
