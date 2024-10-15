use crate::default::DEFAULT_HASH_BUFFER_SIZE;
use blake3::{Hash, Hasher};
use std::fs::File;
use std::io;
use std::path::Path;

/// Culculates the BLAKE3 hash of a byte array.
pub fn calculate_blake3_hash(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    let mut offset = 0;

    // Hash the data in chunks
    while offset < data.len() {
        let end = std::cmp::min(offset + DEFAULT_HASH_BUFFER_SIZE, data.len());
        hasher.update(&data[offset..end]);
        offset = end;
    }
    let hash = hasher.finalize();
    // Return the final hash as a hexadecimal string
    hash.to_hex().to_string()
}

/// Culculates the BLAKE3 hash of a byte array. 
pub fn calculate_blake3_bytes_hash(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    // Hash the data 
    hasher.update(data);
    let hash = hasher.finalize();
    // Return the final hash as a hexadecimal string
    hash.to_hex().to_string()
}

/// Calculates the BLAKE3 hash of a file using file reader.
pub fn calculate_blake3_file_hash(file_path: &Path) -> io::Result<String> {
    let file = File::open(file_path)?;
    let mut hasher = Hasher::new();
    hasher.update_reader(file)?;
    let hash = hasher.finalize();
    // Return the final hash as a hexadecimal string
    Ok(hash.to_hex().to_string())
}

/// Calculates the BLAKE3 hash of a file using memory-mapped file reader.
pub fn calculate_blake3_mmap_file_hash(file_path: &Path) -> io::Result<String> {
    let mut hasher = Hasher::new();
    hasher.update_mmap(file_path)?;
    let hash = hasher.finalize();
    // Return the final hash as a hexadecimal string
    Ok(hash.to_hex().to_string())
}

pub fn blake3_hash(data: &[u8]) -> Hash {
    let mut hasher = Hasher::new();
    let mut offset = 0;

    // Hash the data in chunks
    while offset < data.len() {
        let end = std::cmp::min(offset + DEFAULT_HASH_BUFFER_SIZE, data.len());
        hasher.update(&data[offset..end]);
        offset = end;
    }
    hasher.finalize()
}

/// Culculates the BLAKE3 hash of a byte array. 
pub fn blake3_bytes_hash(data: &[u8]) -> Hash {
    let mut hasher = Hasher::new();
    // Hash the data 
    hasher.update(data);
    hasher.finalize()
}
