use anyhow::Result;
use foctet::core::default::{
    DEFAULT_CACHE_DIR, DEFAULT_LOG_DIR, DEFAULT_OUTPUT_DIR, DEFAULT_TEMP_DIR,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct PathConfig {
    pub temp_dir: PathBuf,
    pub log_dir: PathBuf,
    pub output_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl PathConfig {
    pub fn try_default() -> Result<Self> {
        tracing::debug!("Trying to get path config");
        match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => {
                let temp_dir = user_data_dir.join(DEFAULT_TEMP_DIR);
                let log_dir = user_data_dir.join(DEFAULT_LOG_DIR);
                let output_dir = user_data_dir.join(DEFAULT_OUTPUT_DIR);
                let cache_dir = user_data_dir.join(DEFAULT_CACHE_DIR);
                Ok(Self {
                    temp_dir,
                    log_dir,
                    output_dir,
                    cache_dir,
                })
            }
            None => {
                tracing::error!("Failed to get user data directory path");
                anyhow::bail!("Failed to get user data directory path");
            }
        }
    }
}
