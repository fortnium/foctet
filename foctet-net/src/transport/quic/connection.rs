use super::stream::QuicStream;
use crate::transport::{connection::ConnectionHandle, stream::Stream};
use anyhow::Result;
use foctet_core::{connection::Direction, default, id::NodeId, stream::StreamId};
use quinn::Connection as QuinnConnection;
use stackaddr::StackAddr;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct QuicConnection {
    direction: Direction,
    local_addr: StackAddr,
    remote_addr: StackAddr,
    local_node_id: NodeId,
    remote_node_id: NodeId,
    connection: QuinnConnection,
    pub write_buffer_size: usize,
    pub read_buffer_size: usize,
    next_stream_id: AtomicU32,
}

impl QuicConnection {
    pub fn new(
        direction: Direction,
        local_addr: StackAddr,
        remote_addr: StackAddr,
        connection: QuinnConnection,
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
    pub fn with_connection(mut self, connection: QuinnConnection) -> Self {
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

impl ConnectionHandle for QuicConnection {
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
        let (send_stream, recv_stream) = self.connection.open_bi().await?;
        let stream_id = StreamId::new(self.next_stream_id.fetch_add(1, Ordering::SeqCst));
        Ok(Stream::Quic(QuicStream::new(
            stream_id,
            send_stream,
            recv_stream,
            self.write_buffer_size,
            self.read_buffer_size,
        )))
    }

    async fn accept_stream(&mut self) -> Result<Stream> {
        let (send_stream, recv_stream) = match self.connection.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => return Err(e.into()),
        };
        let stream_id = StreamId::new(self.next_stream_id.fetch_add(1, Ordering::SeqCst));
        Ok(Stream::Quic(QuicStream::new(
            stream_id,
            send_stream,
            recv_stream,
            self.write_buffer_size,
            self.read_buffer_size,
        )))
    }

    async fn close(&mut self) -> Result<()> {
        self.connection.close(0u32.into(), b"");
        Ok(())
    }
}
