//! TLS module for managing certificates and private keys.

pub mod cert;
pub mod key;

use anyhow::Result;
use cert::SkipServerVerification;
use rustls::client::{ClientConfig, WebPkiServerVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::ServerConfig;
use std::path::Path;
use std::sync::Arc;

/// Alias of [`rustls::server::ServerConfig`].
pub type TlsServerConfig = rustls::server::ServerConfig;

/// Alias of [`rustls::client::ClientConfig`].
pub type TlsClientConfig = rustls::client::ClientConfig;

/// Generate a self-signed certificate and private key
/// Returns a tuple of certificate chain and private key in DER format
pub fn generate_self_signed_pair_der(
    subject_alt_names: Vec<String>,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let cert_chain = vec![CertificateDer::from(cert.cert)];
    Ok((cert_chain, key))
}

/// Generate a self-signed certificate and private key
/// Returns a tuple of certificate chain and private key in PEM format
pub fn generate_self_signed_pair_pem(
    subject_alt_names: Vec<String>,
) -> Result<(Vec<String>, String)> {
    let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
    let key = cert.key_pair.serialize_pem();
    let cert_chain = vec![cert.cert.pem()];
    Ok((cert_chain, key))
}

/// Load certificate and private key
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

    /// Create a new TLS configuration with self-signed certificates
    pub fn with_self_signed_certs() -> Result<Self> {
        // ClientConfig
        let native_certs = cert::get_native_certs()?;
        let rustls_client_config = ClientConfig::builder()
            .with_root_certificates(native_certs)
            .with_no_client_auth();
        // ServerConfig
        let (certs, key) = generate_self_signed_pair_der(vec!["localhost".into()])?;
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
        let (certs, key) = generate_self_signed_pair_der(vec!["localhost".into()])?;
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
        let (certs, key) = generate_self_signed_pair_der(vec!["localhost".into()])?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Self {
            client_config: rustls_client_config,
            server_config: rustls_server_config,
        })
    }
}

pub struct TlsServerConfigBuilder  {
    inner: TlsServerConfig,
} 

impl TlsServerConfigBuilder {
    pub fn new_insecure(subject_alt_names: Vec<String>) -> Result<TlsServerConfig> {
        let (certs, key) = generate_self_signed_pair_der(subject_alt_names)?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(rustls_server_config)
    }
    pub fn new_with_cert(cert_path: &Path, key_path: &Path) -> Result<TlsServerConfig> {
        let (certs, key) = load_cert(cert_path, key_path)?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(rustls_server_config)
    }
    pub fn new_with_self_signed_certs(subject_alt_names: Vec<String>) -> Result<TlsServerConfig> {
        let (certs, key) = generate_self_signed_pair_der(subject_alt_names)?;
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(rustls_server_config)
    }
    pub fn with_alpn_protocols(&mut self, protocols: Vec<Vec<u8>>) -> &mut Self {
        self.inner.alpn_protocols = protocols;
        self
    }
}

pub struct TlsClientConfigBuilder  {
    inner: TlsClientConfig,
} 

impl TlsClientConfigBuilder {
    pub fn new_insecure() -> Result<TlsClientConfig> {
        let client_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();
        Ok(client_config)
    }
    pub fn new_with_native_certs() -> Result<TlsClientConfig> {
        let native_certs = cert::get_native_certs()?;
        let client_config = ClientConfig::builder()
            .with_root_certificates(native_certs)
            .with_no_client_auth();
        Ok(client_config)
    }
    pub fn new_with_webpki_verifier(verifier: Arc<WebPkiServerVerifier>) -> Result<TlsClientConfig> {
        let client_config = ClientConfig::builder()
            .with_webpki_verifier(verifier)
            .with_no_client_auth();
        Ok(client_config)
    }
    pub fn with_alpn_protocols(&mut self, protocols: Vec<Vec<u8>>) -> &mut Self {
        self.inner.alpn_protocols = protocols;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_self_signed_pair_der() {
        let (cert_chain, key) = generate_self_signed_pair_der(vec!["localhost".into()]).unwrap();
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key);
        match rustls_server_config {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create ServerConfig: {}", e);
            }
        }
    }

    #[test]
    fn test_generate_self_signed_pair_pem() {
        let (cert_chain, key) = generate_self_signed_pair_pem(vec!["localhost".into()]).unwrap();
        // Save to file
        let cert_path = Path::new("cert.pem");
        let key_path = Path::new("key.pem");
        std::fs::write(cert_path, cert_chain.join("\n")).unwrap();
        std::fs::write(key_path, key).unwrap();
        // Load from file
        let (cert_chain, key) = load_cert(cert_path, key_path).unwrap();
        let rustls_server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key);
        match rustls_server_config {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create ServerConfig: {}", e);
            }
        }
        std::fs::remove_file(cert_path).unwrap();
        std::fs::remove_file(key_path).unwrap();
    }
}
