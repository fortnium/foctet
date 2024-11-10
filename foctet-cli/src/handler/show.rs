use anyhow::Result;
use clap::ArgMatches;

/// Handle the `show` subcommand
/// This will show the global configuration
/// The global configuration is stored in the toml configuration file
pub fn handle(_args: &ArgMatches) -> Result<()> {
    let config = match crate::config::get_config() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Failed to get config: {}", e);
            anyhow::bail!("Failed to get config: {}", e);
        }
    };
    match config.to_string() {
        Ok(config_str) => {
            println!("Current configuration:");
            println!("----------------------");
            println!("{}", config_str);
            println!("----------------------");
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to convert config to string: {}", e);
            anyhow::bail!("Failed to convert config to string: {}", e);
        }
    }
}
