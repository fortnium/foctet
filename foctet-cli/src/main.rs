mod app;
mod cli;
mod config;
mod handler;
mod sys;

use app::AppCommands;
use config::{config_file_exists, generate_default_config};
use std::env;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        app::show_app_desc();
        std::process::exit(0);
    }
    // Check if the configuration file exists, if not generate a default one
    if !config_file_exists() {
        match generate_default_config() {
            Ok(config_path) => {
                tracing::info!(
                    "Generated default configuration file: {}",
                    config_path.display()
                );
            }
            Err(e) => {
                eprintln!("Failed to generate default configuration file: {}", e);
                std::process::exit(1);
            }
        }
    }
    // Load the configuration file
    match config::load_config() {
        Ok(_) => {
            tracing::info!("Configuration file loaded");
        }
        Err(e) => {
            eprintln!("Failed to load configuration file: {}", e);
            std::process::exit(1);
        }
    }
    let cli_command = cli::build_cli();
    let matches = cli_command.get_matches();
    let subcommand_name = matches.subcommand_name().unwrap_or("");
    let app_command = AppCommands::from_str(subcommand_name);
    match app_command {
        Some(AppCommands::Config) => match handler::config::handle(&matches) {
            Ok(_) => {
                tracing::info!("Configuration updated");
            }
            Err(e) => {
                tracing::error!("Failed to update configuration: {}", e);
                std::process::exit(1);
            }
        },
        Some(AppCommands::Show) => match handler::show::handle(&matches) {
            Ok(_) => {
                tracing::info!("Configuration shown");
            }
            Err(e) => {
                tracing::error!("Failed to show configuration: {}", e);
                std::process::exit(1);
            }
        },
        Some(AppCommands::Send) => {
            println!("Send command");
            match handler::send::handle(&matches) {
                Ok(_) => {
                    tracing::info!("File sent");
                }
                Err(e) => {
                    tracing::error!("Failed to send file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(AppCommands::Receive) => {
            println!("Receive command");
            match handler::receive::handle(&matches) {
                Ok(_) => {
                    tracing::info!("File received");
                }
                Err(e) => {
                    tracing::error!("Failed to receive file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(AppCommands::Connect) => {
            println!("Connect command");
            match handler::connect::handle(&matches) {
                Ok(_) => {
                    tracing::info!("Connection closed");
                }
                Err(e) => {
                    tracing::error!("Failed to connect to server: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(AppCommands::Listen) => {
            println!("Listen command");
            match handler::listen::handle(&matches) {
                Ok(_) => {
                    tracing::info!("Server stopped");
                }
                Err(e) => {
                    tracing::error!("Failed to listen for incoming connections: {}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            println!("No command");
        }
    }
}
