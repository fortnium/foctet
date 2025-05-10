//! X.509 certificate handling for foctet
//!
//! This module handles generation, signing, and verification of certificates.

use foctet_core::id::NodeId;
use foctet_core::key;
use std::sync::Arc;
use x509_parser::{prelude::*, signature_algorithm::SignatureAlgorithm};
/// The foctet Public Key Extension is a X.509 extension
/// with the Object Identifier 1.3.6.1.4.1.32473.1.1
const FOCTET_EXT_OID: [u64; 9] = [1, 3, 6, 1, 4, 1, 32473, 1, 1];

/// The node signs the concatenation of the string `foctet-tls-handshake:`
/// and the public key that it used to generate the certificate carrying
/// the foctet Public Key Extension, using its private host key.
/// This signature provides cryptographic proof that the node was
/// in possession of the private host key at the time the certificate was signed.
const FOCTET_SIGNING_PREFIX: [u8; 21] = *b"foctet-tls-handshake:";

// Certificates MUST use the NamedCurve encoding for elliptic curve parameters.
// Similarly, hash functions with an output length less than 256 bits MUST NOT be used.
static FOCTET_SIGNATURE_ALGORITHM: &rcgen::SignatureAlgorithm = &rcgen::PKCS_ECDSA_P256_SHA256;

#[derive(Debug)]
pub(crate) struct AlwaysResolvesCert(Arc<rustls::sign::CertifiedKey>);

impl AlwaysResolvesCert {
    pub(crate) fn new(
        cert: rustls::pki_types::CertificateDer<'static>,
        key: &rustls::pki_types::PrivateKeyDer<'_>,
    ) -> Result<Self, rustls::Error> {
        let certified_key = rustls::sign::CertifiedKey::new(
            vec![cert],
            rustls::crypto::ring::sign::any_ecdsa_type(key)?,
        );
        Ok(Self(Arc::new(certified_key)))
    }
}

impl rustls::client::ResolvesClientCert for AlwaysResolvesCert {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[rustls::SignatureScheme],
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        Some(Arc::clone(&self.0))
    }

    fn has_certs(&self) -> bool {
        true
    }
}

impl rustls::server::ResolvesServerCert for AlwaysResolvesCert {
    fn resolve(
        &self,
        _client_hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        Some(Arc::clone(&self.0))
    }
}

/// Generates a self-signed TLS certificate that includes a foctet-specific
/// certificate extension containing the public key of the given keypair.
pub fn generate(
    identity_keypair: &key::Keypair,
) -> Result<
    (
        rustls::pki_types::CertificateDer<'static>,
        rustls::pki_types::PrivateKeyDer<'static>,
    ),
    GenError,
> {
    let node_id = identity_keypair.public();
    let node_id_base32 = node_id.to_base32();
    let node_id_uri = format!("foctet://{}", node_id_base32);
    let certificate_keypair = rcgen::KeyPair::generate_for(FOCTET_SIGNATURE_ALGORITHM)?;
    let rustls_key = rustls::pki_types::PrivateKeyDer::from(
        rustls::pki_types::PrivatePkcs8KeyDer::from(certificate_keypair.serialize_der()),
    );
    let mut params = rcgen::CertificateParams::new(vec![])?;
    params
        .subject_alt_names
        .push(rcgen::SanType::URI(rcgen::Ia5String::try_from(
            node_id_uri,
        )?));
    params.distinguished_name = rcgen::DistinguishedName::new();

    // Signature start
    //
    let mut signing_msg = Vec::new();
    signing_msg.extend(FOCTET_SIGNING_PREFIX);
    signing_msg.extend(&certificate_keypair.public_key_der());

    let signature = identity_keypair.sign(&signing_msg);

    // ASN.1 encoding
    let signed_extension = yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            writer.next().write_bytes(&node_id.to_bytes());
            writer.next().write_bytes(&signature);
        })
    });

    // Add signed extension to certificate
    params
        .custom_extensions
        .push(rcgen::CustomExtension::from_oid_content(
            &FOCTET_EXT_OID,
            signed_extension,
        ));

    let certificate = params.self_signed(&certificate_keypair)?;

    let rustls_certificate = rustls::pki_types::CertificateDer::from(certificate);

    Ok((rustls_certificate, rustls_key))
}

/// Attempts to parse the provided bytes as a [`FoctetCertificate`].
///
/// For this to succeed, the certificate must contain the specified extension and the signature must
/// match the embedded public key.
pub fn parse<'a>(
    cert: &'a rustls::pki_types::CertificateDer<'a>,
) -> Result<FoctetCertificate<'a>, ParseError> {
    let cert = parse_unverified(cert.as_ref())?;
    cert.verify()?;
    Ok(cert)
}

/// An X.509 certificate with a foctet-specific extension
/// is used to secure foctet connections.
#[derive(Debug)]
pub struct FoctetCertificate<'a> {
    certificate: X509Certificate<'a>,
    extension: FoctetExtension,
}

/// The contents of the specific foctet extension, containing the public host key
/// and a signature performed using the private host key.
#[derive(Debug)]
pub struct FoctetExtension {
    public_key: key::PublicKey,
    /// This signature provides cryptographic proof that the node was
    /// in possession of the private host key at the time the certificate was signed.
    signature: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct GenError(#[from] rcgen::Error);

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ParseError(#[from] pub(crate) webpki::Error);

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct VerificationError(#[from] pub(crate) webpki::Error);

/// Internal function that only parses but does not verify the certificate.
///
/// Useful for testing but unsuitable for production.
fn parse_unverified(der_input: &[u8]) -> Result<FoctetCertificate, webpki::Error> {
    let x509_cert = x509_parser::parse_x509_certificate(der_input)
        .map_err(|_| webpki::Error::BadDer)?
        .1;

    let san_node_id = match x509_cert.subject_alternative_name() {
        Ok(san_opt) => {
            let san = san_opt.ok_or(webpki::Error::BadDer)?;
            let uri = san
                .value
                .general_names
                .iter()
                .find_map(|gn| {
                    if let GeneralName::URI(uri) = gn {
                        Some(uri)
                    } else {
                        None
                    }
                })
                .ok_or(webpki::Error::UnknownIssuer)?;
            let node_id = NodeId::from_base32(&uri["foctet://".len()..])
                .map_err(|_| webpki::Error::UnknownIssuer)?;
            node_id
        }
        Err(e) => {
            tracing::error!("Failed to parse SAN: {:?}", e);
            return Err(webpki::Error::UnknownIssuer);
        }
    };

    let custom_oid = der_parser::oid::Oid::from(&FOCTET_EXT_OID)
        .expect("This is a valid OID of foctet extension");

    let mut extracted_node_id = None;
    let mut extracted_signature = None;

    for ext in x509_cert.extensions() {
        if ext.oid == custom_oid {
            let (node_id_bytes, signature_bytes): (Vec<u8>, Vec<u8>) =
                yasna::decode_der(&ext.value).map_err(|_| webpki::Error::ExtensionValueInvalid)?;
            extracted_node_id = Some(node_id_bytes);
            extracted_signature = Some(signature_bytes);
        }
    }
    let node_id_bytes = extracted_node_id.ok_or(webpki::Error::UnknownIssuer)?;
    let signature = extracted_signature.ok_or(webpki::Error::UnknownIssuer)?;

    let ext_node_id =
        key::PublicKey::try_from_bytes(&node_id_bytes).map_err(|_| webpki::Error::UnknownIssuer)?;
    if san_node_id != ext_node_id {
        return Err(webpki::Error::UnknownIssuer);
    }
    let ext = FoctetExtension {
        public_key: ext_node_id,
        signature,
    };

    Ok(FoctetCertificate {
        certificate: x509_cert,
        extension: ext,
    })
}

impl FoctetCertificate<'_> {
    /// The [`NodeId`] of the remote node.
    pub fn node_id(&self) -> NodeId {
        self.extension.public_key
    }

    /// Verify the `signature` of the `message` signed by the private key corresponding to the
    /// public key stored in the certificate.
    pub fn verify_signature(
        &self,
        signature_scheme: rustls::SignatureScheme,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), VerificationError> {
        let pk = self.public_key(signature_scheme)?;
        pk.verify(message, signature)
            .map_err(|_| webpki::Error::InvalidSignatureForPublicKey)?;

        Ok(())
    }
    /// Get a [`ring::signature::UnparsedPublicKey`] for this `signature_scheme`.
    /// Return `Error` if the `signature_scheme` does not match the public key signature
    /// and hashing algorithm or if the `signature_scheme` is not supported.
    fn public_key(
        &self,
        signature_scheme: rustls::SignatureScheme,
    ) -> Result<ring::signature::UnparsedPublicKey<&[u8]>, webpki::Error> {
        use ring::signature;
        use rustls::SignatureScheme::*;

        let current_signature_scheme = self.signature_scheme()?;
        if signature_scheme != current_signature_scheme {
            // This certificate was signed with a different signature scheme
            return Err(webpki::Error::UnsupportedSignatureAlgorithmForPublicKey);
        }

        let verification_algorithm: &dyn signature::VerificationAlgorithm = match signature_scheme {
            RSA_PKCS1_SHA256 => &signature::RSA_PKCS1_2048_8192_SHA256,
            RSA_PKCS1_SHA384 => &signature::RSA_PKCS1_2048_8192_SHA384,
            RSA_PKCS1_SHA512 => &signature::RSA_PKCS1_2048_8192_SHA512,
            ECDSA_NISTP256_SHA256 => &signature::ECDSA_P256_SHA256_ASN1,
            ECDSA_NISTP384_SHA384 => &signature::ECDSA_P384_SHA384_ASN1,
            ECDSA_NISTP521_SHA512 => {
                // See https://github.com/briansmith/ring/issues/824
                return Err(webpki::Error::UnsupportedSignatureAlgorithm);
            }
            RSA_PSS_SHA256 => &signature::RSA_PSS_2048_8192_SHA256,
            RSA_PSS_SHA384 => &signature::RSA_PSS_2048_8192_SHA384,
            RSA_PSS_SHA512 => &signature::RSA_PSS_2048_8192_SHA512,
            ED25519 => &signature::ED25519,
            ED448 => {
                // See https://github.com/briansmith/ring/issues/463
                return Err(webpki::Error::UnsupportedSignatureAlgorithm);
            }
            // Similarly, hash functions with an output length less than 256 bits
            // MUST NOT be used, due to the possibility of collision attacks.
            // In particular, MD5 and SHA1 MUST NOT be used.
            RSA_PKCS1_SHA1 => return Err(webpki::Error::UnsupportedSignatureAlgorithm),
            ECDSA_SHA1_Legacy => return Err(webpki::Error::UnsupportedSignatureAlgorithm),
            _ => return Err(webpki::Error::UnsupportedSignatureAlgorithm),
        };
        let spki = &self.certificate.tbs_certificate.subject_pki;
        let key = signature::UnparsedPublicKey::new(
            verification_algorithm,
            spki.subject_public_key.as_ref(),
        );

        Ok(key)
    }

    /// This method validates the certificate according to foctet TLS 1.3 specs.
    /// The certificate MUST:
    /// 1. be valid at the time it is received by the node;
    /// 2. use the NamedCurve encoding;
    /// 3. use hash functions with an output length not less than 256 bits;
    /// 4. be self signed;
    /// 5. contain a valid signature in the specific foctet extension.
    fn verify(&self) -> Result<(), webpki::Error> {
        use webpki::Error;
        // The certificate MUST have NotBefore and NotAfter fields set
        // such that the certificate is valid at the time it is received by the node.
        if !self.certificate.validity().is_valid() {
            return Err(Error::InvalidCertValidity);
        }

        // Certificates MUST use the NamedCurve encoding for elliptic curve parameters.
        // Similarly, hash functions with an output length less than 256 bits
        // MUST NOT be used, due to the possibility of collision attacks.
        // In particular, MD5 and SHA1 MUST NOT be used.
        // Endpoints MUST abort the connection attempt if it is not used.
        let signature_scheme = self.signature_scheme()?;
        // Endpoints MUST abort the connection attempt if the certificate’s
        // self-signature is not valid.
        let raw_certificate = self.certificate.tbs_certificate.as_ref();
        let signature = self.certificate.signature_value.as_ref();
        // check if self signed
        self.verify_signature(signature_scheme, raw_certificate, signature)
            .map_err(|_| Error::SignatureAlgorithmMismatch)?;

        // The certificate MUST contain a valid signature in the specific foctet extension.
        let subject_pki = self.certificate.public_key().raw;
        let mut signing_msg = Vec::new();
        signing_msg.extend(FOCTET_SIGNING_PREFIX);
        signing_msg.extend(subject_pki);
        if !self
            .extension
            .public_key
            .verify(&signing_msg, &self.extension.signature)
        {
            return Err(Error::InvalidSignatureForPublicKey);
        }
        Ok(())
    }

    /// Return the signature scheme corresponding to [`AlgorithmIdentifier`]s
    /// of `subject_pki` and `signature_algorithm`
    /// according to <https://www.rfc-editor.org/rfc/rfc8446.html#section-4.2.3>.
    fn signature_scheme(&self) -> Result<rustls::SignatureScheme, webpki::Error> {
        // Certificates MUST use the NamedCurve encoding for elliptic curve parameters.
        // Endpoints MUST abort the connection attempt if it is not used.
        use oid_registry::*;
        use rustls::SignatureScheme::*;

        let signature_algorithm = &self.certificate.signature_algorithm;
        let pki_algorithm = &self.certificate.tbs_certificate.subject_pki.algorithm;

        if pki_algorithm.algorithm == OID_PKCS1_RSAENCRYPTION {
            if signature_algorithm.algorithm == OID_PKCS1_SHA256WITHRSA {
                return Ok(RSA_PKCS1_SHA256);
            }
            if signature_algorithm.algorithm == OID_PKCS1_SHA384WITHRSA {
                return Ok(RSA_PKCS1_SHA384);
            }
            if signature_algorithm.algorithm == OID_PKCS1_SHA512WITHRSA {
                return Ok(RSA_PKCS1_SHA512);
            }
            if signature_algorithm.algorithm == OID_PKCS1_RSASSAPSS {
                // According to https://datatracker.ietf.org/doc/html/rfc4055#section-3.1:
                // Inside of params there should be a sequence of:
                // - Hash Algorithm
                // - Mask Algorithm
                // - Salt Length
                // - Trailer Field

                // We are interested in Hash Algorithm only

                if let Ok(SignatureAlgorithm::RSASSA_PSS(params)) =
                    SignatureAlgorithm::try_from(signature_algorithm)
                {
                    let hash_oid = params.hash_algorithm_oid();
                    if hash_oid == &OID_NIST_HASH_SHA256 {
                        return Ok(RSA_PSS_SHA256);
                    }
                    if hash_oid == &OID_NIST_HASH_SHA384 {
                        return Ok(RSA_PSS_SHA384);
                    }
                    if hash_oid == &OID_NIST_HASH_SHA512 {
                        return Ok(RSA_PSS_SHA512);
                    }
                }

                // Default hash algo is SHA-1, however:
                // In particular, MD5 and SHA1 MUST NOT be used.
                return Err(webpki::Error::UnsupportedSignatureAlgorithm);
            }
        }

        if pki_algorithm.algorithm == OID_KEY_TYPE_EC_PUBLIC_KEY {
            let signature_param = pki_algorithm
                .parameters
                .as_ref()
                .ok_or(webpki::Error::BadDer)?
                .as_oid()
                .map_err(|_| webpki::Error::BadDer)?;
            if signature_param == OID_EC_P256
                && signature_algorithm.algorithm == OID_SIG_ECDSA_WITH_SHA256
            {
                return Ok(ECDSA_NISTP256_SHA256);
            }
            if signature_param == OID_NIST_EC_P384
                && signature_algorithm.algorithm == OID_SIG_ECDSA_WITH_SHA384
            {
                return Ok(ECDSA_NISTP384_SHA384);
            }
            if signature_param == OID_NIST_EC_P521
                && signature_algorithm.algorithm == OID_SIG_ECDSA_WITH_SHA512
            {
                return Ok(ECDSA_NISTP521_SHA512);
            }
            return Err(webpki::Error::UnsupportedSignatureAlgorithm);
        }

        if signature_algorithm.algorithm == OID_SIG_ED25519 {
            return Ok(ED25519);
        }
        if signature_algorithm.algorithm == OID_SIG_ED448 {
            return Ok(ED448);
        }

        Err(webpki::Error::UnsupportedSignatureAlgorithm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_parse_verify() {
        let keypair = key::Keypair::generate();

        let (cert, _) = generate(&keypair).unwrap();
        let parsed_cert = parse(&cert).unwrap();

        assert!(parsed_cert.verify().is_ok());
        assert_eq!(keypair.public(), parsed_cert.node_id());
    }
}
