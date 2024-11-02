mod app;
mod sys;
mod cli;
mod handler;

use app::AppCommands;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        app::show_app_desc();
        std::process::exit(0);
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
