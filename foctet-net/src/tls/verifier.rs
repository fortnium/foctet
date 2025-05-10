//! TLS 1.3 certificates and handshakes handling for foctet
//!
//! This module handles a verification of a client/server certificate chain
//! and signatures allegedly by the given certificates.

use std::sync::Arc;

use foctet_core::id::NodeId;
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::ring::cipher_suite::{
        TLS13_AES_128_GCM_SHA256, TLS13_AES_256_GCM_SHA384, TLS13_CHACHA20_POLY1305_SHA256,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
    CertificateError, DigitallySignedStruct, DistinguishedName, OtherError, SignatureScheme,
    SupportedCipherSuite, SupportedProtocolVersion,
};

use crate::tls::cert;

/// The protocol versions supported by this verifier.
///
/// The spec says:
///
/// > The foctet handshake uses TLS 1.3 (and higher).
/// > Endpoints MUST NOT negotiate lower TLS versions.
pub(crate) static PROTOCOL_VERSIONS: &[&SupportedProtocolVersion] = &[&rustls::version::TLS13];
/// A list of the TLS 1.3 cipher suites supported by rustls.
// By default rustls creates client/server configs with both
// TLS 1.3 __and__ 1.2 cipher suites. But we don't need 1.2.
pub(crate) static CIPHERSUITES: &[SupportedCipherSuite] = &[
    // TLS1.3 suites
    TLS13_CHACHA20_POLY1305_SHA256,
    TLS13_AES_256_GCM_SHA384,
    TLS13_AES_128_GCM_SHA256,
];

/// Implementation of the `rustls` certificate verification traits for foctet.
///
/// Only TLS 1.3 is supported. TLS 1.2 should be disabled in the configuration of `rustls`.
#[derive(Debug)]
pub(crate) struct FoctetCertificateVerifier {
    /// The node ID we intend to connect to
    remote_node_id: Option<NodeId>,
}

/// Foctet requires the following of X.509 server certificate chains:
///
/// - Exactly one certificate must be presented.
/// - The certificate must be self-signed.
/// - The certificate must have a valid foctet extension that includes a signature of its public
///   key.
impl FoctetCertificateVerifier {
    pub(crate) fn new() -> Self {
        Self {
            remote_node_id: None,
        }
    }
    pub(crate) fn with_remote_node_id(remote_node_id: Option<NodeId>) -> Self {
        Self { remote_node_id }
    }

    /// Return the list of SignatureSchemes that this verifier will handle,
    /// in `verify_tls12_signature` and `verify_tls13_signature` calls.
    ///
    /// This should be in priority order, with the most preferred first.
    fn verification_schemes() -> Vec<SignatureScheme> {
        vec![
            // TODO SignatureScheme::ECDSA_NISTP521_SHA512 is not supported by `ring` yet
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            // TODO SignatureScheme::ED448 is not supported by `ring` yet
            SignatureScheme::ED25519,
            // In particular, RSA SHOULD NOT be used unless
            // no elliptic curve algorithms are supported.
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA256,
        ]
    }
}

impl ServerCertVerifier for FoctetCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let node_id = verify_presented_certs(end_entity, intermediates)?;

        if let Some(remote_node_id) = self.remote_node_id {
            // The public host key allows the node to calculate the node ID of the node
            // it is connecting to. Clients MUST verify that the node ID derived from
            // the certificate matches the node ID they intended to connect to,
            // and MUST abort the connection if there is a mismatch.
            if remote_node_id != node_id {
                return Err(rustls::Error::InvalidCertificate(
                    CertificateError::ApplicationVerificationFailure,
                ));
            }
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        unreachable!("`PROTOCOL_VERSIONS` only allows TLS 1.3")
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(cert, dss.scheme, message, dss.signature())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::verification_schemes()
    }
}

/// Foctet requires the following of X.509 client certificate chains:
///
/// - Exactly one certificate must be presented. In particular, client authentication is mandatory
///   in foctet.
/// - The certificate must be self-signed.
/// - The certificate must have a valid foctet extension that includes a signature of its public
///   key.
impl ClientCertVerifier for FoctetCertificateVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        verify_presented_certs(end_entity, intermediates)?;

        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        unreachable!("`PROTOCOL_VERSIONS` only allows TLS 1.3")
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(cert, dss.scheme, message, dss.signature())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::verification_schemes()
    }
}

/// When receiving the certificate chain, an endpoint
/// MUST check these conditions and abort the connection attempt if
/// (a) the presented certificate is not yet valid, OR
/// (b) if it is expired.
/// Endpoints MUST abort the connection attempt if more than one certificate is received,
/// or if the certificate’s self-signature is not valid.
fn verify_presented_certs(
    end_entity: &CertificateDer,
    intermediates: &[CertificateDer],
) -> Result<NodeId, rustls::Error> {
    if !intermediates.is_empty() {
        return Err(rustls::Error::General(
            "foctet-tls requires exactly one certificate".into(),
        ));
    }

    let cert = match cert::parse(end_entity) {
        Ok(cert) => cert,
        Err(_) => {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::BadEncoding,
            ))
        }
    };

    Ok(cert.node_id())
}

fn verify_tls13_signature(
    cert: &CertificateDer,
    signature_scheme: SignatureScheme,
    message: &[u8],
    signature: &[u8],
) -> Result<HandshakeSignatureValid, rustls::Error> {
    match cert::parse(cert)?.verify_signature(signature_scheme, message, signature) {
        Ok(()) => {}
        Err(_) => {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::BadSignature,
            ))
        }
    }

    Ok(HandshakeSignatureValid::assertion())
}

impl From<cert::ParseError> for rustls::Error {
    fn from(cert::ParseError(e): cert::ParseError) -> Self {
        use webpki::Error::*;
        match e {
            BadDer => rustls::Error::InvalidCertificate(CertificateError::BadEncoding),
            e => {
                rustls::Error::InvalidCertificate(CertificateError::Other(OtherError(Arc::new(e))))
            }
        }
    }
}
impl From<cert::VerificationError> for rustls::Error {
    fn from(cert::VerificationError(e): cert::VerificationError) -> Self {
        use webpki::Error::*;
        match e {
            InvalidSignatureForPublicKey => {
                rustls::Error::InvalidCertificate(CertificateError::BadSignature)
            }
            other => rustls::Error::InvalidCertificate(CertificateError::Other(OtherError(
                Arc::new(other),
            ))),
        }
    }
}

/// Dummy certificate verifier that treats any certificate as valid.
/// NOTE, such verification is vulnerable to MITM attacks, but convenient for testing.
#[derive(Debug)]
pub struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    #[allow(dead_code)]
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
