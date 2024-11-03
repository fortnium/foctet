mod app;
mod sys;
mod cli;
mod handler;
mod config;

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
    if !config_file_exists() {
        match generate_default_config() {
            Ok(config_path) => {
                tracing::info!("Generated default configuration file: {}", config_path.display());
            },
            Err(e) => {
                eprintln!("Failed to generate default configuration file: {}", e);
                std::process::exit(1);
            }
        }
    }
    let cli_command = cli::build_cli();
    let matches = cli_command.get_matches();
    let subcommand_name = matches.subcommand_name().unwrap_or("");
    let app_command = AppCommands::from_str(subcommand_name);
    match app_command {
        Some(AppCommands::Config) => {
            println!("Config command");
            handler::config::handle(&matches);
        },
        Some(AppCommands::Send) => {
            println!("Send command");
            handler::send::handle(&matches);
        },
        Some(AppCommands::Receive) => {
            println!("Receive command");
            handler::receive::handle(&matches);
        },
        Some(AppCommands::Connect) => {
            println!("Connect command");
            handler::connect::handle(&matches);
        },
        Some(AppCommands::Listen) => {
            println!("Listen command");
            handler::listen::handle(&matches);
        },
        None => {
            println!("No command");
        },
    }   
}
