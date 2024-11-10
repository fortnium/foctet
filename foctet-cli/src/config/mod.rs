pub mod buffer;
pub mod cache;
pub mod database;
pub mod log;
pub mod network;
pub mod node;
pub mod path;
pub mod timeout;
pub mod tls;

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

use anyhow::Result;
use buffer::BufferConfig;
use cache::CacheConfig;
use database::DatabaseConfig;
use foctet::core::default::DEFAULT_CONFIG_FILE;
use log::LogConfig;
use network::NetworkConfig;
use node::NodeConfig;
use path::PathConfig;
use serde::{Deserialize, Serialize};
use timeout::TimeoutConfig;
use tls::TlsConfig;

pub static CONFIG: OnceLock<Mutex<FoctetConfig>> = OnceLock::new();

#[derive(Serialize, Deserialize, Debug)]
pub struct FoctetConfig {
    pub node: NodeConfig,
    pub tls: TlsConfig,
    pub network: NetworkConfig,
    pub timeout: TimeoutConfig,
    pub buffer: BufferConfig,
    pub log: LogConfig,
    pub path: PathConfig,
    pub database: DatabaseConfig,
    pub cache: CacheConfig,
}

impl FoctetConfig {
    pub fn try_default() -> Result<Self> {
        tracing::debug!("Trying to get default foctet-config");
        Ok(Self {
            node: NodeConfig::try_default()?,
            tls: TlsConfig::try_default()?,
            network: NetworkConfig::default(),
            timeout: TimeoutConfig::default(),
            buffer: BufferConfig::default(),
            log: LogConfig::default(),
            path: PathConfig::try_default()?,
            database: DatabaseConfig::try_default()?,
            cache: CacheConfig::try_default()?,
        })
    }
    pub fn from_file(file_path: &Path) -> Result<Self> {
        let config_str = std::fs::read_to_string(file_path)?;
        let config: Self = toml::from_str(&config_str)?;
        Ok(config)
    }
    pub fn save(&self, file_path: &Path) -> Result<()> {
        let config_str = toml::to_string(self)?;
        std::fs::write(file_path, config_str)?;
        Ok(())
    }
    pub fn save_to_default_file(&self) -> Result<()> {
        let config_path = match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => user_data_dir.join(DEFAULT_CONFIG_FILE),
            None => {
                tracing::error!("Failed to get user data directory path");
                anyhow::bail!("Failed to get user data directory path");
            }
        };
        self.save(&config_path)?;
        Ok(())
    }
    pub fn to_string(&self) -> Result<String> {
        let config_str = toml::to_string(self)?;
        Ok(config_str)
    }
}

pub fn load_config() -> Result<()> {
    let config_path = match foctet::core::fs::get_user_data_dir_path() {
        Some(user_data_dir) => user_data_dir.join(DEFAULT_CONFIG_FILE),
        None => {
            tracing::error!("Failed to get user data directory path");
            anyhow::bail!("Failed to get user data directory path");
        }
    };
    let config = FoctetConfig::from_file(&config_path)?;
    match CONFIG.set(Mutex::new(config)) {
        Ok(_) => {
            tracing::debug!("Config loaded successfully");
        }
        Err(_) => {
            tracing::error!("Failed to load config");
            anyhow::bail!("Failed to load config");
        }
    }
    Ok(())
}

pub fn get_config() -> Result<MutexGuard<'static, FoctetConfig>> {
    let config_path = match foctet::core::fs::get_user_data_dir_path() {
        Some(user_data_dir) => user_data_dir.join(DEFAULT_CONFIG_FILE),
        None => {
            tracing::error!("Failed to get user data directory path");
            anyhow::bail!("Failed to get user data directory path");
        }
    };
    let config = CONFIG.get_or_init(|| {
        tracing::debug!("Config not loaded, loading config");
        let config = FoctetConfig::from_file(&config_path).unwrap();
        Mutex::new(config)
    });
    match config.try_lock() {
        Ok(guard) => Ok(guard),
        Err(_) => {
            tracing::error!("Failed to get config lock");
            anyhow::bail!("Failed to get config lock");
        }
    }
}

pub fn config_file_exists() -> bool {
    match foctet::core::fs::get_user_data_dir_path() {
        Some(user_data_dir) => {
            let config_file_path = user_data_dir.join(DEFAULT_CONFIG_FILE);
            if config_file_path.exists() {
                FoctetConfig::from_file(&config_file_path).is_ok()
            } else {
                false
            }
        }
        None => false,
    }
}

pub fn generate_default_config() -> Result<PathBuf> {
    let config = FoctetConfig::try_default()?;
    match foctet::core::fs::get_user_data_dir_path() {
        Some(user_data_dir) => {
            let config_file_path = user_data_dir.join(DEFAULT_CONFIG_FILE);
            tracing::info!("Save default config to file: {:?}", config_file_path);
            config.save(&config_file_path)?;
            Ok(config_file_path)
        }
        None => {
            tracing::error!("Failed to get user data directory path");
            anyhow::bail!("Failed to get user data directory path");
        }
    }
}
