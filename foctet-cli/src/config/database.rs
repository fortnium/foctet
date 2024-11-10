use anyhow::Result;
use foctet::core::default::{DEFAULT_DATABASE_DIR, DEFAULT_DATABASE_FILE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct DatabaseConfig {
    pub db_path: PathBuf,
}

impl DatabaseConfig {
    pub fn try_default() -> Result<Self> {
        tracing::debug!("Trying to get database config");
        match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => {
                let relative_path: PathBuf =
                    PathBuf::from(DEFAULT_DATABASE_DIR).join(DEFAULT_DATABASE_FILE);
                Ok(Self {
                    db_path: user_data_dir.join(relative_path),
                })
            }
            None => {
                tracing::error!("Failed to get user data directory path");
                anyhow::bail!("Failed to get user data directory path");
            }
        }
    }
}
