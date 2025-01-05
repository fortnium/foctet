use std::net::SocketAddr;
use std::path::PathBuf;

use crate::app::{CRATE_BIN_NAME, CRATE_REPOSITORY};
use clap::{crate_description, crate_version, value_parser};
use clap::{Arg, Command};
use foctet::core::addr::NamedSocketAddr;

pub fn build_cli() -> Command {
    let app_description: &str = crate_description!();
    Command::new(CRATE_BIN_NAME)
        .version(crate_version!())
        .about(format!("{} \n{}", app_description, CRATE_REPOSITORY))
        .allow_external_subcommands(true)
        .subcommand(Command::new("config")
            .about("Set global configuration")
            .arg(Arg::new("node-id")
                .help("Generate a new node ID")
                .long("node-id")
                .num_args(0)
            )
            .arg(Arg::new("cert")
                .help("Path to the certificate file (PEM or DER format)")
                .long("cert")
                .value_name("file-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("key")
                .help("Path to the private key file (PEM or DER format)")
                .long("key")
                .value_name("file-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("bind-addr")
                .help("List of socket addresses to bind to for. Example: 0.0.0.0:4432, [::]:4432")
                .long("bind-addr")
                .value_name("socket-addrs")
                .value_delimiter(',')
                .value_parser(value_parser!(SocketAddr))
            )
            .arg(Arg::new("relay")
                .help("Relay server address. Example:relay.example.com:4433")
                .long("relay")
                .value_name("named-socket-addr")
                .value_parser(value_parser!(NamedSocketAddr))
            )
            .arg(Arg::new("conn-timeout")
                .help("Connection timeout in seconds")
                .long("conn-timeout")
                .value_name("duration-in-seconds")
                .value_parser(value_parser!(u64))
            )
            .arg(Arg::new("read-timeout")
                .help("Read timeout in seconds")
                .long("read-timeout")
                .value_name("duration-in-seconds")
                .value_parser(value_parser!(u64))
            )
            .arg(Arg::new("write-timeout")
                .help("Write timeout in seconds")
                .long("write-timeout")
                .value_name("duration-in-seconds")
                .value_parser(value_parser!(u64))
            )
            .arg(Arg::new("read-buffer-size")
                .help("Read buffer size in bytes")
                .long("read-buffer-size")
                .value_name("size-in-bytes")
                .value_parser(value_parser!(usize))
            )
            .arg(Arg::new("write-buffer-size")
                .help("Write buffer size in bytes")
                .long("write-buffer-size")
                .value_name("size-in-bytes")
                .value_parser(value_parser!(usize))
            )
            .arg(Arg::new("log-level")
                .help("Log level")
                .long("log-level")
                .value_name("level")
                .value_parser(value_parser!(String))
            )
            .arg(Arg::new("temp-dir")
                .help("Temp dir path")
                .long("temp-dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("log-dir")
                .help("Log dir path")
                .long("log-dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("output-dir")
                .help("Output dir path")
                .long("output-dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("cache-dir")
                .help("Cache dir path")
                .long("cache-dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("database")
                .help("Database file path")
                .long("database")
                .value_name("file-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("cache-metadata-dir")
                .help("Metadata cache dir path")
                .long("cache-metadata-dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
            .arg(Arg::new("cache-exp")
                .help("Cache expiration time in seconds")
                .long("cache-exp")
                .value_name("duration-in-seconds")
                .value_parser(value_parser!(u64))
            )
            .arg(Arg::new("cache-max-size")
                .help("Maximum cache size in bytes")
                .long("cache-max-size")
                .value_name("size-in-bytes")
                .value_parser(value_parser!(usize))
            )
        )
        .subcommand(Command::new("show")
            .about("Show global configuration")
        )
        .subcommand(Command::new("send")
            .about("Send a content")
            .arg(Arg::new("file")
                .help("Path to the file to send")
                .long("file")
                .value_name("file-path")
                .value_parser(value_parser!(PathBuf))
                .conflicts_with_all(&["dir", "text"])
            )
            .arg(Arg::new("dir")
                .help("Path to the directory to send")
                .long("dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
                .conflicts_with_all(&["file", "text"])
            )
            .arg(Arg::new("text")
                .help("Text message to send")
                .long("text")
                .value_name("message")
                .value_parser(value_parser!(String))
                .conflicts_with_all(&["file", "dir"])
            )
        )
        .subcommand(Command::new("receive")
            .about("Receive a content")
            .arg(Arg::new("ticket")
                .help("Transfer ticket to receive the content")
                .long("ticket")
                .value_name("transfer-ticket")
                .value_parser(value_parser!(String))
            )
            .arg(Arg::new("dir")
                .help("Path to the directory to receive the content")
                .long("dir")
                .value_name("dir-path")
                .value_parser(value_parser!(PathBuf))
            )
        )
        .subcommand(Command::new("connect")
            .about("Connect to a server")
        )
        .subcommand(Command::new("listen")
            .about("Listen for incoming connections")
        )
}