use crate::hash::Blake3Hash;
use crate::time::UnixTimestamp;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

/// Represents the metadata of the content
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub name: String,
    pub size: usize,
    pub hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ChunkMetadata {
    pub chunk_id: u64,
    pub total_chunks: u64,
    pub chunk_size: usize,
    pub total_size: usize,
    pub offset: usize,
    pub name: String,
}

/// Represents the metadata of the file
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub name: String,
    pub size: usize,
    pub hash: Blake3Hash,
}

impl FileMetadata {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
            Ok((file_metadata, _)) => Ok(file_metadata),
            Err(e) => Err(anyhow::anyhow!("Failed to decode file metadata: {}", e)),
        }
    }
    pub fn to_bytes(&self) -> Result<Bytes> {
        let serialized = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        Ok(Bytes::from(serialized))
    }
}

/// Represents the metadata of the file or compressed directory
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct LocalFileMetadata {
    pub name: String,
    pub path: PathBuf,
    pub size: usize,
    pub hash: Blake3Hash,
    pub is_directory: bool,
    /// File created timestamp in Unix time
    pub created: UnixTimestamp,
    /// File modified timestamp in Unix time
    pub modified: UnixTimestamp,
    /// File accessed timestamp in Unix time
    pub accessed: UnixTimestamp,
}
