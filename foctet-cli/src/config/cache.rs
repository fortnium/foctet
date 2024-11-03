use std::path::PathBuf;

use foctet::core::default::{DEFAULT_CACHE_DIR, DEFAULT_CACHE_EXPIRATION, DEFAULT_CACHE_MAX_SIZE};
use serde::{Deserialize, Serialize};

/// Cache configuration for managing cache behavior and storage
#[derive(Serialize, Deserialize, Debug)]
pub struct CacheConfig {
    /// Path to the metadata cache directory
    pub metadata_cache_dir: PathBuf,
    /// Cache expiration time in seconds
    pub expiration_seconds: u64,
    /// Maximum cache size in bytes
    pub max_size_bytes: u64,
}

impl CacheConfig {
    /// Create a new cache configuration with default values
    pub fn try_default() -> anyhow::Result<Self> {
        tracing::debug!("Trying to get cache config");
        match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => {
                let cache_dir = user_data_dir.join(DEFAULT_CACHE_DIR);
                Ok(Self {
                    metadata_cache_dir: cache_dir,
                    expiration_seconds: DEFAULT_CACHE_EXPIRATION,
                    max_size_bytes: DEFAULT_CACHE_MAX_SIZE,
                })
            }
            None => {
                tracing::error!("Failed to get user data directory path");
                anyhow::bail!("Failed to get user data directory path");
            }
        }
    }
}
