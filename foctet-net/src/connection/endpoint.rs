//! Module for creating QUIC endpoints.

use crate::tls::cert::SkipServerVerification;
use anyhow::Result;
use foctet_core::default::DEFAULT_KEEP_ALIVE_INTERVAL;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use quinn_proto::crypto::rustls::QuicClientConfig;
use rustls::pki_types::CertificateDer;
use rustls::ClientConfig as RustlsClientConfig;
use rustls::ServerConfig as RustlsServerConfig;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

/// Create quinn client config from rustls client config
pub fn make_client_config(client_config: RustlsClientConfig) -> Result<ClientConfig> {
    Ok(ClientConfig::new(Arc::new(QuicClientConfig::try_from(
        client_config,
    )?)))
}

/// Create quinn server config from rustls server config
pub fn make_server_config(server_config: RustlsServerConfig) -> Result<ServerConfig> {
    let mut quic_server_config =
        ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_config)?));
    let transport_config = Arc::get_mut(&mut quic_server_config.transport).unwrap();
    transport_config.keep_alive_interval(Some(DEFAULT_KEEP_ALIVE_INTERVAL));
    Ok(quic_server_config)
}

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
/// - bind_addr: the address to bind the client endpoint to.
///
/// - server_certs: list of trusted certificates.
pub fn make_client_endpoint(bind_addr: SocketAddr, server_certs: &[&[u8]]) -> Result<Endpoint> {
    let client_cfg = configure_client(server_certs)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

/// Constructs a QUIC client endpoint using root certificates found in the platform's native certificate store.
pub fn make_native_client_endpoint(bind_addr: SocketAddr) -> Result<Endpoint> {
    let native_certs = crate::tls::cert::get_native_certs()?;
    let rustls_client_config = RustlsClientConfig::builder()
        .with_root_certificates(native_certs)
        .with_no_client_auth();
    let client_config =
        ClientConfig::new(Arc::new(QuicClientConfig::try_from(rustls_client_config)?));
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

/// Constructs a QUIC client endpoint that skips server certificate verification.
///
/// ## Args
///
/// - bind_addr: the address to bind the client endpoint to.
pub fn make_insecure_client_endpoint(bind_addr: SocketAddr) -> Result<Endpoint> {
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(ClientConfig::new(Arc::new(QuicClientConfig::try_from(
        RustlsClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth(),
    )?)));

    Ok(endpoint)
}

/// Constructs a QUIC endpoint configured to listen for incoming connections on a certain address
/// and port.
/// If `cert_path` and `key_path` are provided, the server will use the certificate and key at those
/// paths. Otherwise, a self-signed certificate will be generated.
pub fn make_server_endpoint(
    bind_addr: SocketAddr,
    cert_path: &Path,
    key_path: &Path,
) -> Result<Endpoint> {
    let server_config = configure_server(cert_path, key_path)?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok(endpoint)
}

/// Constructs a QUIC endpoint configured to listen for incoming connections on a certain address
/// and port.
pub fn make_self_signed_server_endpoint(bind_addr: SocketAddr) -> Result<Endpoint> {
    let server_config = configure_self_signed_server()?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok(endpoint)
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(server_certs: &[&[u8]]) -> Result<ClientConfig> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(CertificateDer::from(*cert))?;
    }

    Ok(ClientConfig::with_root_certificates(Arc::new(certs))?)
}

/// Returns server configuration along with its certificate.
fn configure_server(cert_path: &Path, key_path: &Path) -> Result<ServerConfig> {
    let (cert_chain, key) = crate::tls::load_cert(cert_path, key_path)?;
    let mut server_config = ServerConfig::with_single_cert(cert_chain, key)?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok(server_config)
}

/// Returns default server configuration along with its certificate.
fn configure_self_signed_server() -> Result<ServerConfig> {
    let (cert_chain, key) = crate::tls::generate_self_signed_pair_der(vec!["localhost".into()])?;

    let mut server_config = ServerConfig::with_single_cert(cert_chain, key)?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok(server_config)
}
