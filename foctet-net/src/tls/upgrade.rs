use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use super::{
    cert::{self, FoctetCertificate},
    config::{make_client_config, make_server_config},
};
use anyhow::Result;
use foctet_core::{id::NodeId, key::Keypair};
use rustls::CommonState;
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

#[derive(Clone)]
pub struct ConnectionUpgrader {
    tls_acceptor: TlsAcceptor,
    tls_connector: TlsConnector,
}

impl ConnectionUpgrader {
    pub fn new(identity: &Keypair) -> Result<Self, cert::GenError> {
        let server_config = make_server_config(identity)?;
        let client_config = make_client_config(identity, None)?;
        Ok(Self {
            tls_acceptor: TlsAcceptor::from(Arc::new(server_config)),
            tls_connector: TlsConnector::from(Arc::new(client_config)),
        })
    }
    pub fn with_config(
        server_config: rustls::ServerConfig,
        client_config: rustls::ClientConfig,
    ) -> Self {
        Self {
            tls_acceptor: TlsAcceptor::from(Arc::new(server_config)),
            tls_connector: TlsConnector::from(Arc::new(client_config)),
        }
    }
    pub async fn upgrade_inbound(
        &self,
        raw_stream: TcpStream,
    ) -> Result<(NodeId, TlsStream<TcpStream>)> {
        let tls_stream = TlsStream::Server(self.tls_acceptor.accept(raw_stream).await?);
        let node_id = extract_single_certificate(tls_stream.get_ref().1)?.node_id();
        Ok((node_id, tls_stream))
    }
    pub async fn upgrade_outbound(
        &self,
        raw_stream: TcpStream,
    ) -> Result<(NodeId, TlsStream<TcpStream>)> {
        // Spec: In order to keep this flexibility for future versions, clients that only
        // support the version of the handshake defined in this document MUST NOT send any value
        // in the Server Name Indication. Setting `ServerName` to unspecified will
        // disable the use of the SNI extension.
        let name = ServerName::IpAddress(rustls::pki_types::IpAddr::from(IpAddr::V4(
            Ipv4Addr::UNSPECIFIED,
        )));
        let tls_stream = TlsStream::Client(self.tls_connector.connect(name, raw_stream).await?);
        let node_id = extract_single_certificate(tls_stream.get_ref().1)?.node_id();
        Ok((node_id, tls_stream))
    }
}

fn extract_single_certificate(
    state: &CommonState,
) -> Result<FoctetCertificate<'_>, cert::ParseError> {
    let Some([cert]) = state.peer_certificates() else {
        panic!("config enforces exactly one certificate");
    };
    cert::parse(cert)
}
