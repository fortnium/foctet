//! TLS module for managing certificates and private keys.

pub mod cert;
pub mod key;

use cert::SkipServerVerification;
use rustls::client::ClientConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::ServerConfig;
use std::path::Path;
use anyhow::Result;

/// Generate a self-signed certificate and private key
pub fn generate_self_signed_pair(
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let cert_chain = vec![CertificateDer::from(cert.cert)];
    Ok((cert_chain, key))
}

/// Load or generate certificate and private key
pub fn load_cert(
    cert_path: &Path,
    key_path: &Path,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert_chain = cert::load_certs(cert_path)?;
    let key = key::load_key(key_path)?;
    Ok((cert_chain, key))
}

/// The TLS configuration includes both client and server configurations
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub client_config: ClientConfig,
    pub server_config: ServerConfig,
}

impl TlsConfig {
    /// Create a new TLS configuration with the specified certificate and private key
    pub fn with_cert(cert_path: &Path, key_path: &Path) -> Result<Self> {
        // ClientConfig
        let native_certs = cert::get_native_certs()?;
        let rustls_client_config = ClientConfig::builder()
            .with_root_certificates(native_certs)
            .with_no_client_auth();
        // ServerConfig
        let (certs, key) = load_cert(cert_path, key_path)?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Self {
            client_config: rustls_client_config,
            server_config: rustls_server_config,
        })
    }

    /// Create a new TLS configuration with system certificates
    pub fn with_self_signed_certs() -> Result<Self> {
        // ClientConfig
        let native_certs = cert::get_native_certs()?;
        let rustls_client_config = ClientConfig::builder()
            .with_root_certificates(native_certs)
            .with_no_client_auth();
        // ServerConfig
        let (certs, key) = generate_self_signed_pair()?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Self {
            client_config: rustls_client_config,
            server_config: rustls_server_config,
        })
    }
    /// Create a new TLS configuration with system certificates
    pub fn new_native_config() -> Result<Self> {
        // ClientConfig
        let native_certs = cert::get_native_certs()?;
        let rustls_client_config = ClientConfig::builder()
            .with_root_certificates(native_certs)
            .with_no_client_auth();
        // ServerConfig
        let (certs, key) = generate_self_signed_pair()?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Self {
            client_config: rustls_client_config,
            server_config: rustls_server_config,
        })
    }
    /// Create a new TLS configuration with no certificate verification
    pub fn new_insecure_config() -> Result<Self> {
        // ClientConfig
        let rustls_client_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();
        // ServerConfig
        let (certs, key) = generate_self_signed_pair()?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Self {
            client_config: rustls_client_config,
            server_config: rustls_server_config,
        })
    }
}