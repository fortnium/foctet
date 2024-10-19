use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use anyhow::Result;
use crate::config::SocketConfig;

pub struct TcpConnection {
    pub stream: TlsStream<TcpStream>,
    // TODO: add more fields and methods
}

pub struct TcpSocket {
    config: SocketConfig,
    pub tls_connector: TlsConnector,
    pub tls_acceptor: TlsAcceptor,
    pub connections: HashMap<u64, TcpConnection>,
    next_connection_id: u64,
    // TODO: add more fields and methods
}

impl TcpSocket {
    pub fn new(config: SocketConfig) -> Self {
        let tls_connector = TlsConnector::from(Arc::new(config.tls_config.client_config.clone()));
        let tls_acceptor = TlsAcceptor::from(Arc::new(config.tls_config.server_config.clone()));
        Self {
            config: config,
            tls_connector: tls_connector,
            tls_acceptor: tls_acceptor,
            connections: HashMap::new(),
            next_connection_id: 0,
        }
    }

    pub async fn connect(&mut self, server_addr: SocketAddr, server_name: &str) -> Result<u64> {
        let name = rustls_pki_types::ServerName::try_from(server_name.to_string())?;
        let stream = TcpStream::connect(server_addr).await?;
        let tls_stream = self.tls_connector.connect(name, stream).await?;
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        self.connections.insert(id, TcpConnection { stream: TlsStream::Client(tls_stream) });
        Ok(id)
    }

    pub async fn listen(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.config.server_addr).await?;
        while let Ok((stream, _addr)) = listener.accept().await {
            let tls_stream = self.tls_acceptor.accept(stream).await?;
            let id = self.next_connection_id;
            self.next_connection_id += 1;
            self.connections.insert(id, TcpConnection { stream: TlsStream::Server(tls_stream) });
        }
        Ok(())
    }

    pub fn remove_connection(&mut self, id: u64) {
        self.connections.remove(&id);
    }

}
