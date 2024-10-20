use crate::default::{DEFAULT_HASH_BUFFER_SIZE, DEFAULT_HASH_LEN};
use serde::{Serialize, Deserialize};
use blake3::{Hash, Hasher};
use std::fs::File;
use std::io;
use std::path::Path;
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Blake3Hash([u8; DEFAULT_HASH_LEN]);

impl Blake3Hash {
    pub fn new(hash: [u8; DEFAULT_HASH_LEN]) -> Self {
        Self(hash)
    }
    pub fn from_blake3_hash(hash: Hash) -> Self {
        Self(*hash.as_bytes())
    }
    pub fn from_hex(hex: &str) -> Self {
        let bytes = hex::decode(hex).unwrap_or_default();
        let mut hash = [0; DEFAULT_HASH_LEN];
        hash.copy_from_slice(&bytes);
        Self(hash)
    }
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
    pub fn to_bytes(&self) -> [u8; DEFAULT_HASH_LEN] {
        self.0
    }
    pub fn zero() -> Self {
        Self([0; DEFAULT_HASH_LEN])
    }
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub fn calculate_hash(data: &[u8]) -> Result<Blake3Hash> {
    let hash = calculate_blake3_bytes_hash(data)?;
    Ok(Blake3Hash::from_blake3_hash(hash))  
}

pub fn calculate_file_hash(file_path: &Path) -> Result<Blake3Hash> {
    let hash = calculate_blake3_file_hash(file_path)?;
    Ok(Blake3Hash::from_blake3_hash(hash))
}

/// Culculates the BLAKE3 hash of a byte array. 
fn calculate_blake3_bytes_hash(data: &[u8]) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    // Hash the data 
    hasher.update(data);
    Ok(hasher.finalize())
}

/// Culculates the BLAKE3 hash of a byte array in chunks.
#[allow(dead_code)]
fn calculate_blake3_hash(data: &[u8]) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    let mut offset = 0;

    // Hash the data in chunks
    while offset < data.len() {
        let end = std::cmp::min(offset + DEFAULT_HASH_BUFFER_SIZE, data.len());
        hasher.update(&data[offset..end]);
        offset = end;
    }
    Ok(hasher.finalize())
}

/// Calculates the BLAKE3 hash of a file using file reader.
fn calculate_blake3_file_hash(file_path: &Path) -> io::Result<Hash> {
    let file = File::open(file_path)?;
    let mut hasher = Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

/// Calculates the BLAKE3 hash of a file using memory-mapped file reader.
#[allow(dead_code)]
fn calculate_blake3_mmap_file_hash(file_path: &Path) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    hasher.update_mmap(file_path)?;
    Ok(hasher.finalize())
}
