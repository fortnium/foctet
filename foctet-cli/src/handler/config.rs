use std::path::PathBuf;

use clap::ArgMatches;
use anyhow::Result;
use foctet::core::key::NodeKeyPair;

pub fn handle(args: &ArgMatches) -> Result<()> {
    let mut config = match crate::config::get_config() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Failed to get config: {}", e);
            anyhow::bail!("Failed to get config: {}", e);
        }
    };
    let sub_args = match args.subcommand_matches("config") {
        Some(matches) => matches,
        None => {
            tracing::error!("Failed to get subcommand matches");
            anyhow::bail!("Failed to get subcommand matches");
        },
    };
    if sub_args.get_flag("node-id") {
        let key_pair = NodeKeyPair::generate();
        match key_pair.save_to_default_file() {
            Ok(path) => {
                config.node.node_id = key_pair.public_key().to_base64();
                config.node.key_pair_path = path;
            },
            Err(e) => {
                tracing::error!("Failed to save node key pair: {}", e);
                anyhow::bail!("Failed to save node key pair: {}", e);
            }
        }
    }
    if let Some(cert_path) = sub_args.get_one::<PathBuf>("cert") {
        config.tls.cert_path = cert_path.to_path_buf();
    }
    config.save_to_default_file()?;
    Ok(())
}
