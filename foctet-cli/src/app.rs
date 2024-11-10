use crate::sys;
use clap::{crate_description, crate_name, crate_version};

// APP information
pub const CRATE_BIN_NAME: &str = "foctet";
pub const CRATE_UPDATE_DATE: &str = "2024-11-02";
pub const CRATE_REPOSITORY: &str = "https://github.com/shellrow/foctet";

pub enum AppCommands {
    Config,
    Show,
    Send,
    Receive,
    Connect,
    Listen,
}

impl AppCommands {
    pub fn from_str(s: &str) -> Option<AppCommands> {
        match s {
            "config" => Some(AppCommands::Config),
            "show" => Some(AppCommands::Show),
            "send" => Some(AppCommands::Send),
            "receive" => Some(AppCommands::Receive),
            "connect" => Some(AppCommands::Connect),
            "listen" => Some(AppCommands::Listen),
            _ => None,
        }
    }
}

pub fn show_app_desc() {
    println!(
        "{} v{} ({}) {}",
        crate_name!(),
        crate_version!(),
        CRATE_UPDATE_DATE,
        sys::get_os_type()
    );
    println!("{}", crate_description!());
    println!("{}", CRATE_REPOSITORY);
    println!();
    println!("'{} --help' for more information.", CRATE_BIN_NAME);
    println!();
}

#[allow(unused)]
pub fn show_banner_with_starttime() {
    println!(
        "{} v{} {}",
        crate_name!(),
        crate_version!(),
        sys::get_os_type()
    );
    println!("{}", CRATE_REPOSITORY);
    println!();
    println!("Starting at {}", sys::get_sysdate());
    println!();
}

#[allow(unused)]
pub fn exit_with_error_message(message: &str) {
    println!();
    println!("Error: {}", message);
    std::process::exit(1);
}

#[allow(unused)]
pub fn show_error_with_help(message: &str) {
    println!();
    println!("Error: {}", message);
    println!();
    println!("'{} --help' for more information.", CRATE_BIN_NAME);
    println!();
}
