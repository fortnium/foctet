use anyhow::Result;
use foctet::core::default::{DEFAULT_KEYPAIR_FILE, DEFAULT_KEYS_DIR};
use foctet::core::key::NodeKeyPair;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeConfig {
    pub node_id: String,
    pub key_pair_path: PathBuf,
}

impl NodeConfig {
    pub fn try_default() -> Result<Self> {
        tracing::debug!("Try to get node config");
        // 1. Get user data directory path
        let user_data_dir: PathBuf = match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => user_data_dir,
            None => {
                anyhow::bail!("Failed to get user data directory path");
            }
        };
        // 2. Check key directory exists
        let relative_path: PathBuf = PathBuf::from(DEFAULT_KEYS_DIR);
        let key_pair_dir_path = user_data_dir.join(relative_path);
        if !key_pair_dir_path.exists() {
            match std::fs::create_dir_all(&key_pair_dir_path) {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Failed to create key pair directory: {}", e);
                    anyhow::bail!("Failed to create key pair directory: {}", e);
                }
            }
        }
        // 3. Check key pair exists
        let key_pair_path = key_pair_dir_path.join(DEFAULT_KEYPAIR_FILE);
        if !key_pair_path.exists() {
            // Generate key pair
            let key_pair = NodeKeyPair::generate();
            key_pair.save_to_default_file()?;
            Ok(Self {
                node_id: key_pair.public_key().to_base32()?,
                key_pair_path: key_pair_path,
            })
        } else {
            let key_pair = NodeKeyPair::load_from_default_file()?;
            Ok(Self {
                node_id: key_pair.public_key().to_base32()?,
                key_pair_path: key_pair_path,
            })
        }
    }
}
