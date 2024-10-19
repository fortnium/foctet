use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use foctet_core::frame::Frame;
use tokio::sync::mpsc;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use anyhow::Result;
use crate::config::SocketConfig;
use super::{endpoint, ConnectionState, FoctetStream};

pub struct QuicStream {
    pub send_stream: SendStream,
    pub recv_stream: RecvStream,
}

impl FoctetStream for QuicStream {
    async fn send_data(&mut self, stream_id: u64, data: &[u8]) -> Result<()> {
        // TODO: Implement
        Ok(())
    }

    async fn receive_data(&mut self, stream_id: u64, buffer: &mut [u8]) -> Result<usize> {
        // TODO: Implement
        Ok(0)
    }

    async fn send_frame(&mut self, stream_id: u64, frame: Frame) -> Result<()> {
        // TODO: Implement
        Ok(())
    }

    async fn receive_frame(&mut self, stream_id: u64) -> Result<usize> {
        // TODO: Implement
        Ok(0)
    }

    async fn send_file(&self, stream_id: u64, file_path: &std::path::Path) -> Result<()> {
        // TODO: Implement
        Ok(())
    }

    async fn receive_file(&self, stream_id: u64, file_path: &std::path::Path) -> Result<u64> {
        // TODO: Implement
        Ok(0)
    }

    fn close(&mut self) -> Result<()> {
        self.send_stream.finish()?;
        Ok(())
    }
}

pub struct QuicConnection {
    pub connection_id: u64,
    /// The QUIC connection
    pub connection: Connection,
    /// The streams of the QUIC connection
    /// Key: Stream ID
    /// Value: The QUIC stream (SendStream, RecvStream)
    pub streams: HashMap<u64, QuicStream>,
    pub state: ConnectionState,
}

pub struct QuicSocket {
    pub config: SocketConfig,
    pub endpoint: Endpoint,
    pub connections: HashMap<u64, Arc<QuicConnection>>,
    next_connection_id: u64,
}

impl QuicSocket {
    pub fn new(config: SocketConfig) -> Result<Self> {
        let client_config: ClientConfig = endpoint::make_client_config(config.tls_config.client_config.clone())?;
        let server_config: ServerConfig = endpoint::make_server_config(config.tls_config.server_config.clone())?;
        let mut endpoint: Endpoint = Endpoint::server(server_config, config.server_addr)?;
        endpoint.set_default_client_config(client_config);
        Ok(Self {
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
        self.connections.insert(id, Arc::new(QuicConnection {
            connection_id: id,
            connection,
            streams: HashMap::new(),
            state: ConnectionState::Connected,
        }));
        Ok(id)
    }

    pub async fn listen(&mut self, sender: mpsc::Sender<Arc<QuicConnection>>) -> Result<()> {
        while let Some(incoming) = self.endpoint.accept().await {
            match incoming.await {
                Ok(connection) => {
                    let id = self.next_connection_id;
                    let quic_connection = Arc::new(QuicConnection {
                        connection_id: id,
                        connection,
                        streams: HashMap::new(),
                        state: ConnectionState::Connected,
                    });
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

    pub fn remove_connection(&mut self, id: u64) {
        self.connections.remove(&id);
    }
    // TODO: add more methods
}