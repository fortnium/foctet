use crate::default::{DEFAULT_HASH_BUFFER_SIZE, DEFAULT_HASH_LEN};
use anyhow::Result;
use blake3::{Hash, Hasher};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::thread;

const CHUNK_THRESHOLD: usize = 1 * 1024 * 1024;
const MMAP_THRESHOLD: u64 = 64 * 1024 * 1024;

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

/// Calculates the hash for a byte array.
/// If the size of the array exceeds the `CHUNK_THRESHOLD`,
/// the data is processed in chunks.
pub fn calculate_hash(data: &[u8]) -> Result<Blake3Hash> {
    let hash = if data.len() > CHUNK_THRESHOLD {
        calc_blake3_bytes_hash_chunk(data)?
    } else {
        calc_blake3_bytes_hash(data)?
    };
    Ok(Blake3Hash::from_blake3_hash(hash))
}

/// Culculates the BLAKE3 hash of a file.
/// If the file size exceeds the `MMAP_THRESHOLD`,
/// reading the contents of a file using memory mapping
/// is faster than reading the file using a file reader.
/// If the number of available threads is greater than 1,
/// the hash is calculated using multiple threads.
pub fn calculate_file_hash(file_path: &Path) -> Result<Blake3Hash> {
    let file_size = fs::metadata(file_path)?.len();

    let hash = if file_size > MMAP_THRESHOLD {
        let parallelism = thread::available_parallelism()?.get();
        if parallelism > 1 {
            calc_blake3_file_hash_mmap_multithread(file_path)?
        } else {
            calc_blake3_file_hash_mmap(file_path)?
        }
    } else {
        calc_blake3_file_hash(file_path)?
    };

    Ok(Blake3Hash::from_blake3_hash(hash))
}

/// Culculates the BLAKE3 hash of a byte array.
fn calc_blake3_bytes_hash(data: &[u8]) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    // Hash the data
    hasher.update(data);
    Ok(hasher.finalize())
}

/// Culculates the BLAKE3 hash of a byte array in chunks.
fn calc_blake3_bytes_hash_chunk(data: &[u8]) -> io::Result<Hash> {
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
fn calc_blake3_file_hash(file_path: &Path) -> io::Result<Hash> {
    let file = File::open(file_path)?;
    let mut hasher = Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

/// Calculates the BLAKE3 hash of a file using memory-mapped file reader.
fn calc_blake3_file_hash_mmap(file_path: &Path) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    hasher.update_mmap(file_path)?;
    Ok(hasher.finalize())
}

/// Calculates the BLAKE3 hash of a file using memory-mapped file reader with multiple threads.
fn calc_blake3_file_hash_mmap_multithread(file_path: &Path) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    hasher.update_mmap_rayon(file_path)?;
    Ok(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Write, path::PathBuf};

    const TEST_FILE_SMALL: &str = "test_small.txt";
    const TEST_FILE_LARGE: &str = "test_large.txt";
    // 128MB
    const LARGE_FILE_SIZE: usize = 128 * 1024 * 1024; 

    fn create_test_file(file_name: &str, size: usize) -> PathBuf {
        let file_path = PathBuf::from(file_name);
        let mut file = File::create(&file_path).unwrap();
        let data = vec![b'a'; size];
        file.write_all(&data).unwrap();
        file_path
    }

    #[test]
    fn test_calculate_hash_small_data() {
        let data = b"hello world";
        let hash = calculate_hash(data).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
    }

    #[test]
    fn test_calculate_hash_large_data() {
        let data = vec![b'a'; CHUNK_THRESHOLD + 1];
        let hash = calculate_hash(&data).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
    }

    #[test]
    fn test_calculate_file_hash_small_file() {
        let data = b"hello world";
        let file_path = create_test_file(TEST_FILE_SMALL, data.len());
        let hash = calculate_file_hash(&file_path).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_calculate_file_hash_large_file() {
        let file_path = create_test_file(TEST_FILE_LARGE, LARGE_FILE_SIZE);
        let hash = calculate_file_hash(&file_path).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_calculate_file_hash_nonexistent() {
        let file_path = Path::new("nonexistent.txt");
        let result = calculate_file_hash(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_hash_empty_data() {
        let data: &[u8] = &[];
        let hash = calculate_hash(data).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
    }

    #[test]
    fn test_calculate_file_hash_empty_file() {
        let file_path = create_test_file(TEST_FILE_SMALL, 0);
        let hash = calculate_file_hash(&file_path).unwrap();
        assert_eq!(hash.len(), DEFAULT_HASH_LEN);
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_calculate_file_hash_parallelism() {
        let file_path = create_test_file(TEST_FILE_LARGE, LARGE_FILE_SIZE);
        let threads = thread::available_parallelism().unwrap();
        if threads.get() > 1 {
            let hash = calculate_file_hash(&file_path).unwrap();
            assert_eq!(hash.len(), DEFAULT_HASH_LEN);
        }
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_hash_consistency() {
        let data = b"consistent data";
        let hash1 = calculate_hash(data).unwrap();
        let hash2 = calculate_hash(data).unwrap();
        assert_eq!(hash1.to_hex(), hash2.to_hex());
    }

    #[test]
    fn test_hash_different_inputs() {
        let data1 = b"data1";
        let data2 = b"data2";
        let hash1 = calculate_hash(data1).unwrap();
        let hash2 = calculate_hash(data2).unwrap();
        assert_ne!(hash1.to_hex(), hash2.to_hex());
    }
}
