use anyhow::Result;
use foctet::core::default::{DEFAULT_CERT_FILE, DEFAULT_KEY_FILE, DEFAULT_TLS_DIR};
use foctet::net::tls;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TlsConfig {
    pub fn try_default() -> Result<Self> {
        tracing::debug!("Trying to get tls config");
        match foctet::core::fs::get_user_data_dir_path() {
            Some(user_data_dir) => {
                let tls_dir = user_data_dir.join(DEFAULT_TLS_DIR);
                if !tls_dir.exists() {
                    match std::fs::create_dir_all(&tls_dir) {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Failed to create tls directory: {}", e);
                            anyhow::bail!("Failed to create tls directory: {}", e);
                        }
                    }
                }
                let cert_path = tls_dir.join(DEFAULT_CERT_FILE);
                let key_path = tls_dir.join(DEFAULT_KEY_FILE);
                if cert_path.exists() && key_path.exists() {
                    return Ok(Self {
                        cert_path,
                        key_path,
                    });
                } else {
                    let (cert_chain, key) =
                        tls::generate_self_signed_pair_pem(vec!["localhost".into()])?;
                    // Save to file
                    std::fs::write(&cert_path, cert_chain.join("\n"))?;
                    std::fs::write(&key_path, key)?;
                    Ok(Self {
                        cert_path,
                        key_path,
                    })
                }
            }
            None => {
                tracing::error!("Failed to get user data directory path");
                anyhow::bail!("Failed to get user data directory path");
            }
        }
    }
}
