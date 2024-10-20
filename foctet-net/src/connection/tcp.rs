use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use foctet_core::frame::{ContentId, Frame};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use anyhow::Result;
use crate::config::SocketConfig;

use super::{ConnectionState, FoctetStream};

pub struct TlsTcpStream {
    pub stream: TlsStream<TcpStream>,
}

impl FoctetStream for TlsTcpStream {
    async fn send_data(&mut self, data: &[u8], content_id: Option<ContentId>) -> Result<()> {
        Ok(())
    }

    async fn receive_data(&mut self, buffer: &mut Vec<u8>, content_id: Option<ContentId>) -> Result<usize> {
        // TODO: Implement
        Ok(0)
    }

    async fn send_frame(&mut self, frame: Frame) -> Result<()> {
        // TODO: Implement
        Ok(())
    }

    async fn receive_frame(&mut self, content_id: Option<ContentId>) -> Result<Frame> {
        // TODO: Implement
        Ok(Frame::empty())
    }

    async fn send_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<()> {
        // TODO: Implement
        Ok(())
    }

    async fn receive_file(&mut self, file_path: &std::path::Path, content_id: Option<ContentId>) -> Result<u64> {
        // TODO: Implement
        Ok(0)
    }

    async fn close(&mut self) -> Result<()> {
        self.stream.get_mut().0.shutdown().await?;
        Ok(())
    }
}

pub struct TcpConnection {
    pub connection_id: u64,
    pub stream: TlsStream<TcpStream>,
    pub state: ConnectionState,
}

pub struct TcpSocket {
    config: SocketConfig,
    pub tls_connector: TlsConnector,
    pub tls_acceptor: TlsAcceptor,
    pub connections: HashMap<u64, Arc<TcpConnection>>,
    next_connection_id: u64,
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
        self.connections.insert(id, Arc::new(TcpConnection { connection_id: id, stream: TlsStream::Client(tls_stream), state: ConnectionState::Connected }));
        Ok(id)
    }

    pub async fn listen(&mut self, sender: mpsc::Sender<Arc<TcpConnection>>) -> Result<()> {
        let listener = TcpListener::bind(self.config.server_addr).await?;
        while let Ok((stream, _addr)) = listener.accept().await {
            let tls_stream = self.tls_acceptor.accept(stream).await?;
            let id = self.next_connection_id;
            let tcp_connection = Arc::new(TcpConnection { connection_id: id, stream: TlsStream::Server(tls_stream), state: ConnectionState::Connected });
            let tcp_connection_clone = Arc::clone(&tcp_connection);
            self.connections.insert(id, tcp_connection);
            self.next_connection_id += 1;
            sender.send(tcp_connection_clone).await?;
        }
        Ok(())
    }

    pub fn get_connection(&self, id: u64) -> Option<Arc<TcpConnection>> {
        self.connections.get(&id).cloned()
    }

    pub fn get_all_connections(&self) -> Vec<Arc<TcpConnection>> {
        self.connections.values().cloned().collect()
    }

    pub fn remove_connection(&mut self, id: u64) {
        self.connections.remove(&id);
    }

}
