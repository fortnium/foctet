//! Ed25519 keys.
use anyhow::Result;
use base32::Alphabet;
use core::fmt;
use ed25519_dalek::{self as ed25519, Signer as _, Verifier as _};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use zeroize::Zeroize;

/// An Ed25519 keypair.
#[derive(Clone)]
pub struct Keypair(ed25519::SigningKey);

impl Keypair {
    /// Generate a new random Ed25519 keypair.
    pub fn generate() -> Keypair {
        Keypair::from(SecretKey::generate())
    }

    /// Convert the keypair into a byte array by concatenating the bytes
    /// of the secret scalar and the compressed public point,
    /// an informal standard for encoding Ed25519 keypairs.
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_keypair_bytes()
    }

    /// Try to parse a keypair from the [binary format](https://datatracker.ietf.org/doc/html/rfc8032#section-5.1.5)
    /// produced by [`Keypair::to_bytes`], zeroing the input on success.
    ///
    /// Note that this binary format is the same as `ed25519_dalek`'s and `ed25519_zebra`'s.
    pub fn try_from_bytes(kp: &mut [u8]) -> Result<Keypair> {
        let bytes = <[u8; 64]>::try_from(&*kp)?;
        match ed25519::SigningKey::from_keypair_bytes(&bytes) {
            Ok(k) => {
                kp.zeroize();
                Ok(Keypair(k))
            }
            Err(e) => {
                kp.zeroize();
                Err(e.into())
            }
        }
    }

    /// Sign a message using the private key of this keypair.
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        self.0.sign(msg).to_bytes().to_vec()
    }

    /// Get the public key of this keypair.
    pub fn public(&self) -> PublicKey {
        PublicKey {
            public_key: self.0.verifying_key().to_bytes(),
        }
    }

    /// Get the secret key of this keypair.
    pub fn secret(&self) -> SecretKey {
        SecretKey {
            secret_key: self.0.to_bytes(),
        }
    }
}

impl fmt::Debug for Keypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keypair")
            .field("public", &self.0.verifying_key())
            .finish()
    }
}

/// Demote an Ed25519 keypair to a secret key.
impl From<Keypair> for SecretKey {
    fn from(kp: Keypair) -> SecretKey {
        SecretKey {
            secret_key: kp.0.to_bytes(),
        }
    }
}

/// Promote an Ed25519 secret key into a keypair.
impl From<SecretKey> for Keypair {
    fn from(sk: SecretKey) -> Keypair {
        let signing = ed25519::SigningKey::from_bytes(&sk.secret_key);
        Keypair(signing)
    }
}

/// An Ed25519 public key.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct PublicKey {
    public_key: [u8; ed25519::PUBLIC_KEY_LENGTH],
}

impl PublicKey {
    /// Generates a new `PublicKey` instance.
    pub fn generate() -> Self {
        Keypair::generate().public()
    }
    /// Create a public key with all zero bytes.
    pub fn zero() -> PublicKey {
        PublicKey {
            public_key: [0u8; 32],
        }
    }
    /// Check if the public key is all zero bytes.
    pub fn is_zero(&self) -> bool {
        self.public_key.iter().all(|&b| b == 0)
    }
    /// Verify the Ed25519 signature on a message using the public key.
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        match ed25519::Signature::try_from(sig) {
            Ok(sig) => match ed25519::VerifyingKey::from_bytes(&self.public_key) {
                Ok(vkey) => vkey.verify(msg, &sig).is_ok(),
                Err(e) => {
                    tracing::error!("Failed to parse public key: {:?}", e);
                    false
                }
            },
            Err(e) => {
                tracing::error!("Failed to parse signature: {:?}", e);
                false
            }
        }
    }

    /// Convert the public key to a byte array in compressed form
    pub fn to_bytes(&self) -> [u8; 32] {
        self.public_key
    }

    /// Try to parse a public key from a byte array containing
    /// the actual key as produced by `to_bytes`.
    pub fn try_from_bytes(k: &[u8]) -> Result<PublicKey> {
        let k = <[u8; 32]>::try_from(k)?;
        let _vkey = ed25519::VerifyingKey::from_bytes(&k)?;
        Ok(PublicKey { public_key: k })
    }
    /// Converts a RFC4648 base32 string into a ContentId.
    pub fn from_base32(encoded: &str) -> Result<PublicKey> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or(anyhow::anyhow!("Invalid base32"))?;
        PublicKey::try_from_bytes(&decoded)
    }
    /// Converts the ContentId to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> String {
        base32::encode(Alphabet::Rfc4648 { padding: false }, &self.to_bytes())
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_base32().fmt(f)
    }
}

impl FromStr for PublicKey {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        PublicKey::from_base32(s)
    }
}

/// An Ed25519 secret key.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecretKey {
    secret_key: [u8; ed25519::SECRET_KEY_LENGTH],
}

/// View the bytes of the secret key.
impl AsRef<[u8]> for SecretKey {
    fn as_ref(&self) -> &[u8] {
        &self.secret_key[..]
    }
}

impl SecretKey {
    /// Generate a new Ed25519 secret key.
    pub fn generate() -> SecretKey {
        let mut csprng = rand::rngs::OsRng;
        let signing = ed25519::SigningKey::generate(&mut csprng);
        SecretKey {
            secret_key: signing.to_bytes(),
        }
    }

    /// Try to parse an Ed25519 secret key from a byte slice
    /// containing the actual key, zeroing the input on success.
    /// If the bytes do not constitute a valid Ed25519 secret key, an error is returned.
    pub fn try_from_bytes(mut sk_bytes: impl AsMut<[u8]>) -> Result<SecretKey> {
        let sk_bytes = sk_bytes.as_mut();
        let secret = <[u8; 32]>::try_from(&*sk_bytes)?;
        sk_bytes.zeroize();
        Ok(SecretKey { secret_key: secret })
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.secret_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq_keypairs(kp1: &Keypair, kp2: &Keypair) -> bool {
        kp1.public() == kp2.public() && kp1.0.to_bytes() == kp2.0.to_bytes()
    }

    #[test]
    fn ed25519_keypair_encode_decode() {
        let kp1 = Keypair::generate();
        let mut kp1_enc = kp1.to_bytes();
        let kp2 = Keypair::try_from_bytes(&mut kp1_enc).unwrap();
        assert!(eq_keypairs(&kp1, &kp2));
        assert!(kp1_enc.iter().all(|b| *b == 0));
    }

    #[test]
    fn ed25519_keypair_from_secret() {
        let kp1 = Keypair::generate();
        let mut sk = kp1.0.to_bytes();
        let kp2 = Keypair::from(SecretKey::try_from_bytes(&mut sk).unwrap());
        assert!(eq_keypairs(&kp1, &kp2));
        assert_eq!(sk, [0u8; 32]);
    }

    #[test]
    fn ed25519_signature() {
        let kp = Keypair::generate();
        let pk = kp.public();

        let msg = "hello world".as_bytes();
        let sig = kp.sign(msg);
        assert!(pk.verify(msg, &sig));

        let mut invalid_sig = sig.clone();
        invalid_sig[3..6].copy_from_slice(&[10, 23, 42]);
        assert!(!pk.verify(msg, &invalid_sig));

        let invalid_msg = "h3ll0 w0rld".as_bytes();
        assert!(!pk.verify(invalid_msg, &sig));
    }
}
