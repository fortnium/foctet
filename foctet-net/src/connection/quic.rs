use std::{collections::HashMap, net::SocketAddr};

use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use anyhow::Result;
use crate::config::SocketConfig;

use super::endpoint;

pub struct QuicStream {
    pub send_stream: SendStream,
    pub recv_stream: RecvStream,
}

pub struct QuicConnection {
    /// The QUIC connection
    pub connection: Connection,
    /// The streams of the QUIC connection
    /// Key: Stream ID
    /// Value: The QUIC stream (SendStream, RecvStream)
    pub streams: HashMap<u64, QuicStream>,
    // TODO: add more fields and methods
}

pub struct QuicSocket {
    pub config: SocketConfig,
    pub endpoint: Endpoint,
    pub connections: HashMap<u64, QuicConnection>,
    next_connection_id: u64,
    // TODO: add more fields
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
        self.connections.insert(id, QuicConnection {
            connection,
            streams: HashMap::new(),
        });
        Ok(id)
    }

    pub async fn listen(&mut self) -> Result<()> {
        while let Some(incoming) = self.endpoint.accept().await {
            match incoming.await {
                Ok(connection) => {
                    let id = self.next_connection_id;
                    self.next_connection_id += 1;
                    self.connections.insert(id, QuicConnection {
                        connection,
                        streams: HashMap::new(),
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {:?}", e);
                }
            };
        }
        Ok(())
    }

    pub fn remove_connection(&mut self, id: u64) {
        self.connections.remove(&id);
    }
    // TODO: add more methods
}