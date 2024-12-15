mod client;
mod config;
mod handler;
mod message;
mod server;

use std::{net::SocketAddr, path::PathBuf};
use config::ServerConfig;
use foctet::core::node::NodeId;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use anyhow::Result;

/// Command line arguments for the file sender.
#[derive(Parser, Debug)]
struct Args {
    /// Server address to bind to.
    //#[clap(default_value = "0.0.0.0:4432")]
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server address to bind to.",
        default_value = "0.0.0.0:4432"
    )]
    server_addr: SocketAddr,

    /// The server name for subject-alt-names (SANs) in the certificate. 
    #[arg(
        short = 'n',
        long = "name",
        help = "The server name for subject-alt-names (SANs) in the certificate. ",
        default_value = "localhost"
    )]
    server_name: String,

    /// Path to the certificate file (PEM or DER format).
    #[arg(
        short = 'c',
        long = "cert",
        help = "Path to the certificate file (PEM or DER format)."
    )]
    cert_path: Option<PathBuf>,

    /// Path to the private key file (PEM or DER format).
    #[arg(
        short = 'k',
        long = "key",
        help = "Path to the private key file (PEM or DER format)."
    )]
    key_path: Option<PathBuf>,

    /// Insecure mode to use self-signed certificate and skip certificate verification.
    #[arg(short, long, help = "Insecure mode to use self-signed certificate and skip certificate verification.")]
    insecure: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // A builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse command line arguments
    let _args = Args::parse();

    let node_id = NodeId::generate();
    let config = ServerConfig::new(node_id);

    let mut server = server::Server::spawn(config).await?;
    tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => (),
        _ = server.handle() => (),
    }

    server.shutdown().await
}
