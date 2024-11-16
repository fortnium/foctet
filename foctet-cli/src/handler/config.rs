use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use clap::ArgMatches;
use foctet::core::{addr::NamedSocketAddr, key::NodeKeyPair};

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
        }
    };
    // [node]
    if sub_args.get_flag("node-id") {
        let key_pair = NodeKeyPair::generate();
        match key_pair.save_to_default_file() {
            Ok(path) => {
                config.node.node_id = key_pair.public_key().to_base32()?;
                config.node.key_pair_path = path;
            }
            Err(e) => {
                tracing::error!("Failed to save node key pair: {}", e);
                anyhow::bail!("Failed to save node key pair: {}", e);
            }
        }
    }
    // [tls]
    if let Some(cert_path) = sub_args.get_one::<PathBuf>("cert") {
        config.tls.cert_path = cert_path.to_path_buf();
    }
    if let Some(key_path) = sub_args.get_one::<PathBuf>("key") {
        config.tls.key_path = key_path.to_path_buf();
    }
    // [network]
    if let Some(bind_addrs) = sub_args.get_many::<SocketAddr>("bind-addr") {
        config.network.bind_addrs = bind_addrs.cloned().collect();
    }
    if let Some(relay_addr) = sub_args.get_one::<NamedSocketAddr>("relay") {
        config.network.relay_addr = relay_addr.clone();
    }
    // [timeout]
    if let Some(conn_timeout) = sub_args.get_one::<u64>("conn-timeout") {
        config.timeout.conn_timeout = *conn_timeout;
    }
    if let Some(read_timeout) = sub_args.get_one::<u64>("read-timeout") {
        config.timeout.read_timeout = *read_timeout;
    }
    if let Some(write_timeout) = sub_args.get_one::<u64>("write-timeout") {
        config.timeout.write_timeout = *write_timeout;
    }
    // [buffer]
    if let Some(read_buffer_size) = sub_args.get_one::<usize>("read-buffer-size") {
        config.buffer.read_buffer_size = *read_buffer_size;
    }
    if let Some(write_buffer_size) = sub_args.get_one::<usize>("write-buffer-size") {
        config.buffer.write_buffer_size = *write_buffer_size;
    }
    // [log]
    if let Some(log_level) = sub_args.get_one::<String>("log-level") {
        config.log.log_level = log_level.to_string();
    }
    // [path]
    if let Some(temp_path) = sub_args.get_one::<PathBuf>("temp-dir") {
        config.path.temp_dir = temp_path.to_path_buf();
    }
    if let Some(log_path) = sub_args.get_one::<PathBuf>("log-dir") {
        config.path.log_dir = log_path.to_path_buf();
    }
    if let Some(output_path) = sub_args.get_one::<PathBuf>("output-dir") {
        config.path.output_dir = output_path.to_path_buf();
    }
    if let Some(cache_path) = sub_args.get_one::<PathBuf>("cache-dir") {
        config.path.cache_dir = cache_path.to_path_buf();
    }
    // [database]
    if let Some(db_path) = sub_args.get_one::<PathBuf>("database") {
        config.database.db_path = db_path.to_path_buf();
    }
    // [cache]
    if let Some(metadata_cache_dir) = sub_args.get_one::<PathBuf>("cache-metadata-dir") {
        config.cache.metadata_cache_dir = metadata_cache_dir.to_path_buf();
    }
    if let Some(expiration_seconds) = sub_args.get_one::<u64>("cache-exp") {
        config.cache.expiration_seconds = *expiration_seconds;
    }
    if let Some(max_size_bytes) = sub_args.get_one::<u64>("cache-max-size") {
        config.cache.max_size_bytes = *max_size_bytes;
    }
    // Save the updated configuration
    config.save_to_default_file()?;
    Ok(())
}
