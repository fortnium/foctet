use anyhow::anyhow;
use anyhow::Result;
use base32::Alphabet;
use ring::pkcs8::Document;
use ring::rand::SystemRandom;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;
use ring::signature::ED25519_PUBLIC_KEY_LEN;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use ttl_cache::TtlCache;
use uuid::Uuid;

use crate::hash::Blake3Hash;

pub const UUID_V4_BYTES_LEN: usize = 16;

pub fn generate_key_pair() -> Result<Ed25519KeyPair> {
    let rng = SystemRandom::new();
    let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| anyhow!(e))?;
    Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref()).map_err(|e| anyhow!(e))
}

/// Generate PKCS#8 document
pub fn generate_key_pair_doc() -> Result<Document> {
    let rng = SystemRandom::new();
    let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| anyhow!(e))?;
    Ok(pkcs8_doc)
}

pub fn load_key_pair(pkcs8_bytes: &[u8]) -> Result<Ed25519KeyPair> {
    Ed25519KeyPair::from_pkcs8(pkcs8_bytes).map_err(|e| anyhow!(e))
}

pub fn load_key_pair_from_file(path: &Path) -> Result<Ed25519KeyPair> {
    let pkcs8_bytes = fs::read(path).map_err(|e| anyhow!(e))?;
    load_key_pair(&pkcs8_bytes)
}

pub fn verify_signature(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let public_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
    public_key.verify(message, signature).is_ok()
}

/// key cache capacity for public keys.
const KEY_CACHE_CAPACITY: usize = 1024 * 16;

// The key cache is a TTL cache that stores public keys using the node ID (BLAKE3 hash of the public key) as the key.
pub static KEY_CACHE: OnceLock<Mutex<TtlCache<[u8; ED25519_PUBLIC_KEY_LEN], NodePublicKey>>> =
    OnceLock::new();

fn lock_key_cache() -> MutexGuard<'static, TtlCache<[u8; ED25519_PUBLIC_KEY_LEN], NodePublicKey>> {
    let mutex = KEY_CACHE.get_or_init(|| Mutex::new(TtlCache::new(KEY_CACHE_CAPACITY)));
    mutex.lock().expect("failed to lock key cache")
}

/* fn try_lock_key_cache() -> Result<MutexGuard<'static, TtlCache<String, PublicKey>>, FoctetError> {
    let mutex = KEY_CACHE.get_or_init(|| Mutex::new(TtlCache::new(KEY_CACHE_CAPACITY)));
    match mutex.try_lock() {
        Ok(guard) => Ok(guard),
        Err(_) => Err(FoctetError::new_with_message("failed to lock key cache")),
    }
} */

/// A ED25519 public key. This is a wrapper around a byte array of length 32.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodePublicKey {
    public_key: [u8; ED25519_PUBLIC_KEY_LEN],
}

impl NodePublicKey {
    /// Generates a new `PublicKey` instance.
    pub fn generate() -> Self {
        let rng = SystemRandom::new();
        let key_pair = Ed25519KeyPair::generate_pkcs8(&rng).expect("Failed to generate key pair");
        let key_pair =
            Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).expect("Failed to parse key pair");
        let bytes = key_pair.public_key().as_ref();
        let mut public_key = [0; ED25519_PUBLIC_KEY_LEN];
        public_key.copy_from_slice(bytes);
        NodePublicKey {
            public_key: public_key,
        }
    }

    /// Loads a `PublicKey` from a given byte slice.
    pub fn from_bytes(public_key_bytes: &[u8]) -> Self {
        let public_key =
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key_bytes);
        let mut key = [0; ED25519_PUBLIC_KEY_LEN];
        key.copy_from_slice(public_key.as_ref());
        NodePublicKey { public_key: key }
    }

    /// Returns the public key as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.public_key
    }

    /// Returns the BLAKE3 hash of the public key
    pub fn hash(&self) -> Result<Blake3Hash> {
        crate::hash::calculate_hash(&self.public_key)
    }

    /// Returns the BLAKE3 hash of the public key, as a hexadecimal string.
    pub fn hash_hex(&self) -> Result<String> {
        self.hash().map(|hash| hash.to_hex())
    }

    /// Returns the public key as a hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.public_key)
    }

    /// Caches the public key using the node ID as the key.
    pub fn cache(&self) {
        let mut cache = lock_key_cache();
        cache.insert(
            self.public_key,
            self.clone(),
            Duration::from_secs(60 * 60 * 24 * 7),
        );
    }

    /// Tries to get the `PublicKey` from the cache.
    pub fn from_cache(key: [u8; ED25519_PUBLIC_KEY_LEN]) -> Option<Self> {
        let cache = lock_key_cache();
        cache.get(&key).map(|public_key| public_key.clone())
    }

    /// The zero public key.
    /// This is used as a placeholder for an empty public key.
    pub fn zero() -> Self {
        NodePublicKey {
            public_key: [0; ED25519_PUBLIC_KEY_LEN],
        }
    }
    /// Check if the public key is zero.
    pub fn is_zero(&self) -> bool {
        self.public_key.iter().all(|&x| x == 0)
    }
    /// Get the public key length.
    pub fn len(&self) -> usize {
        self.public_key.len()
    }
    /// Converts a RFC4648 base32 string into a ContentId.
    pub fn from_base32(encoded: &str) -> Result<Self> {
        let decoded = base32::decode(Alphabet::Rfc4648 { padding: false }, encoded)
            .ok_or_else(|| anyhow::anyhow!("Failed to decode base32 string"))?;
        let node_addr: Self = bincode::deserialize(&decoded)?;
        Ok(node_addr)
    }
    /// Converts the ContentId to a single RFC4648 base32 string.
    pub fn to_base32(&self) -> Result<String> {
        let serialized = bincode::serialize(self)?;
        Ok(base32::encode(Alphabet::Rfc4648 { padding: false }, &serialized))
    }
}

/// A ED25519 key pair. This is a wrapper around bytes of a key pair. (PKCS#8 document)
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeKeyPair {
    key_pair: Vec<u8>,
}

impl NodeKeyPair {
    /// Generates a new `NodeKeyPair` instance.
    pub fn generate() -> Self {
        let rng = SystemRandom::new();
        let key_pair = Ed25519KeyPair::generate_pkcs8(&rng).expect("Failed to generate key pair");
        NodeKeyPair {
            key_pair: key_pair.as_ref().to_vec(),
        }
    }

    /// Loads a `KeyPair` from a given byte slice of PKCS#8 document.
    pub fn from_bytes(key_pair_bytes: &[u8]) -> Result<Self> {
        load_key_pair(key_pair_bytes).map(|_key_pair| NodeKeyPair {
            key_pair: key_pair_bytes.to_vec(),
        })
    }

    /// Returns the key pair as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.key_pair
    }

    /// Returns the public key of the key pair.
    pub fn public_key(&self) -> NodePublicKey {
        let key_pair =
            Ed25519KeyPair::from_pkcs8(self.key_pair.as_ref()).expect("Failed to parse key pair");
        let bytes = key_pair.public_key().as_ref();
        NodePublicKey::from_bytes(bytes)
    }

    /// Signs a message using the key pair.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let key_pair =
            Ed25519KeyPair::from_pkcs8(self.key_pair.as_ref()).expect("Failed to parse key pair");
        key_pair.sign(message).as_ref().to_vec()
    }

    /// Saves the key pair to a file.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            fs::create_dir_all(path.parent().unwrap()).map_err(|e| anyhow!(e))?;
        }
        fs::write(path, &self.key_pair).map_err(|e| anyhow!(e))
    }

    /// Saves the key pair to a default file.
    pub fn save_to_default_file(&self) -> Result<PathBuf> {
        let relative_path: PathBuf = PathBuf::from(crate::default::DEFAULT_KEYS_DIR)
            .join(crate::default::DEFAULT_KEYPAIR_FILE);
        let path = crate::fs::get_user_data_path(&relative_path)
            .ok_or_else(|| anyhow!("failed to get user file path"))?;
        match self.save_to_file(&path) {
            Ok(_) => Ok(path),
            Err(e) => Err(e),
        }
    }

    /// Loads a key pair from a file.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let pkcs8_bytes = fs::read(path).map_err(|e| anyhow!(e))?;
        NodeKeyPair::from_bytes(&pkcs8_bytes)
    }

    /// Loads a key pair from a default file.
    pub fn load_from_default_file() -> Result<Self> {
        let relative_path: PathBuf = PathBuf::from(crate::default::DEFAULT_KEYS_DIR)
            .join(crate::default::DEFAULT_KEYPAIR_FILE);
        let path = crate::fs::get_user_data_path(&relative_path)
            .ok_or_else(|| anyhow!("failed to get user file path"))?;
        NodeKeyPair::load_from_file(&path)
    }
}

/// Generate a new UUID (Universally Unique Identifier) byte array.
/// 128-bit (16 bytes) UUID v4 is used.
pub fn generate_uuid_v4_bytes() -> [u8; UUID_V4_BYTES_LEN] {
    let unique_id: Uuid = Uuid::new_v4();
    let mut id = [0u8; UUID_V4_BYTES_LEN];
    id.copy_from_slice(unique_id.as_bytes());
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_pair() {
        let key_pair = generate_key_pair().unwrap();
        let public_key = key_pair.public_key().as_ref();
        assert_eq!(public_key.len(), ED25519_PUBLIC_KEY_LEN);
    }

    #[test]
    fn test_load_key_pair() {
        let key_doc = generate_key_pair_doc().unwrap();
        let pkcs8_bytes = key_doc.as_ref();
        let key_pair = load_key_pair(&pkcs8_bytes).unwrap();
        let public_key = key_pair.public_key().as_ref();
        assert_eq!(public_key.len(), ED25519_PUBLIC_KEY_LEN);
    }

    #[test]
    fn test_load_key_pair_from_file() {
        let key_doc = generate_key_pair_doc().unwrap();
        let pkcs8_bytes = key_doc.as_ref();
        let path = Path::new("./test_key_pair.p8");
        fs::write(path, pkcs8_bytes).unwrap();
        let key_pair = load_key_pair_from_file(path).unwrap();
        let public_key = key_pair.public_key().as_ref();
        assert_eq!(public_key.len(), ED25519_PUBLIC_KEY_LEN);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_verify_signature() {
        let key_pair = generate_key_pair().unwrap();
        let public_key = key_pair.public_key().as_ref();
        let message = b"hello world";
        let signature = key_pair.sign(message);
        assert!(verify_signature(public_key, message, &signature.as_ref()));
    }

    #[test]
    fn test_public_key() {
        let key_pair = generate_key_pair().unwrap();
        let public_key_bytes = key_pair.public_key().as_ref();
        let public_key = NodePublicKey::from_bytes(public_key_bytes);
        assert_eq!(public_key.as_bytes(), public_key.public_key.as_ref());
        assert_eq!(
            public_key.hash().unwrap(),
            crate::hash::calculate_hash(public_key.as_bytes()).unwrap()
        );
        public_key.cache();
        let cached_public_key = NodePublicKey::from_cache(public_key.public_key).unwrap();
        assert_eq!(cached_public_key.as_bytes(), public_key.as_bytes());
    }

    #[test]
    fn test_public_key_size() {
        let public_key = NodePublicKey::generate();
        assert_eq!(public_key.len(), ED25519_PUBLIC_KEY_LEN);
        println!("Public key bytes: {:?}", public_key.public_key);
        println!("Public key bytes len: {} bytes", public_key.len());
        println!("Public key hex: {}", public_key.to_hex());
        println!("Public key hex len: {}", public_key.to_hex().len());
        let hash = public_key.hash().unwrap();
        println!("Hash: {:?}", hash.to_bytes());
        println!("Hash len: {}", hash.len());
        println!("Hash hex: {}", hash.to_hex());
        println!("Hash hex len: {}", hash.to_hex().len());
    }

    #[test]
    fn test_key_pair() {
        let key_pair = NodeKeyPair::generate();
        let public_key = key_pair.public_key();
        let message = b"hello world";
        let signature = key_pair.sign(message);
        assert!(verify_signature(public_key.as_bytes(), message, &signature));
        let key_pair_bytes = key_pair.as_bytes();
        let loaded_key_pair = NodeKeyPair::from_bytes(key_pair_bytes).unwrap();
        assert_eq!(loaded_key_pair.as_bytes(), key_pair_bytes);
        let path = Path::new("./test_key_pair.p8");
        key_pair.save_to_file(path).unwrap();
        let loaded_key_pair = NodeKeyPair::load_from_file(path).unwrap();
        assert_eq!(loaded_key_pair.as_bytes(), key_pair.as_bytes());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_base32_key() {
        let key_pair = NodeKeyPair::generate();
        let public_key = key_pair.public_key();
        let encoded = public_key.to_base32().unwrap();
        let decoded_public_key = NodePublicKey::from_base32(&encoded).unwrap();
        println!("Public key: {:?}", public_key.as_bytes());
        println!("Encoded: {}", encoded);
        println!("Decoded: {:?}", decoded_public_key.as_bytes());
        assert_eq!(decoded_public_key.as_bytes(), public_key.as_bytes());
    }
}
