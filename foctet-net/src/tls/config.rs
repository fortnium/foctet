use std::sync::Arc;

use foctet_core::id::NodeId;
use foctet_core::key::Keypair;

use super::cert::AlwaysResolvesCert;
use super::{cert, verifier};

const FOCTET_ALPN: [u8; 6] = *b"foctet";

/// Create a TLS client configuration for foctet.
pub fn make_client_config(
    keypair: &Keypair,
    remote_node_id: Option<NodeId>,
) -> Result<rustls::ClientConfig, cert::GenError> {
    let (certificate, private_key) = cert::generate(keypair)?;

    let mut provider = rustls::crypto::ring::default_provider();
    provider.cipher_suites = verifier::CIPHERSUITES.to_vec();

    let cert_resolver = Arc::new(
        AlwaysResolvesCert::new(certificate, &private_key)
            .expect("Client cert key DER is valid; qed"),
    );

    let mut crypto = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_protocol_versions(verifier::PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured; qed")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(
            verifier::FoctetCertificateVerifier::with_remote_node_id(remote_node_id),
        ))
        .with_client_cert_resolver(cert_resolver);
    crypto.alpn_protocols = vec![FOCTET_ALPN.to_vec()];

    Ok(crypto)
}

/// Create a TLS server configuration for foctet.
pub fn make_server_config(keypair: &Keypair) -> Result<rustls::ServerConfig, cert::GenError> {
    let (certificate, private_key) = cert::generate(keypair)?;

    let mut provider = rustls::crypto::ring::default_provider();
    provider.cipher_suites = verifier::CIPHERSUITES.to_vec();

    let cert_resolver = Arc::new(
        AlwaysResolvesCert::new(certificate, &private_key)
            .expect("Server cert key DER is valid; qed"),
    );

    let mut crypto = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_protocol_versions(verifier::PROTOCOL_VERSIONS)
        .expect("Cipher suites and kx groups are configured; qed")
        .with_client_cert_verifier(Arc::new(verifier::FoctetCertificateVerifier::new()))
        .with_cert_resolver(cert_resolver);
    crypto.alpn_protocols = vec![FOCTET_ALPN.to_vec()];

    Ok(crypto)
}
