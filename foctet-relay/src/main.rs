mod client;
mod config;
mod message;
mod server;

use std::{net::SocketAddr, path::PathBuf};
use config::ServerConfig;
use foctet::{core::{default::{DEFAULT_RELAY_PACKET_QUEUE_CAPACITY, DEFAULT_RELAY_SERVER_CHANNEL_CAPACITY}, node::NodeId}, net::config::EndpointConfig};
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

    /// The packet queue capacity.
    #[arg(
        short = 'q',
        long = "queue",
        help = "The packet queue capacity.",
        default_value = DEFAULT_RELAY_PACKET_QUEUE_CAPACITY.to_string(),
    )]
    packet_queue_capacity: usize,

    /// The server channel capacity.
    #[arg(
        short = 's',
        long = "channel",
        help = "The server channel capacity.",
        default_value = DEFAULT_RELAY_SERVER_CHANNEL_CAPACITY.to_string(),
    )]
    server_channel_capacity: usize,
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
    let args = Args::parse();

    // TODO: Add support for config file.
    let node_id = NodeId::generate();
    let endpoint_config: EndpointConfig = EndpointConfig::new()
        .with_server_addr(args.server_addr)
        .with_cert_path_option(args.cert_path)
        .with_key_path_option(args.key_path)
        .with_insecure(args.insecure);

    let config = ServerConfig::new(node_id)
        .with_server_name(args.server_name)
        .with_endpoint_config(endpoint_config)
        .with_packet_queue_capacity(args.packet_queue_capacity)
        .with_server_channel_capacity(args.server_channel_capacity);

    tracing::info!("Relay Node Addr: {:?}", config.relay_addr().to_base32());

    let mut server = server::Server::spawn(config).await?;

    tracing::info!("Server listening on {}", args.server_addr);
    tracing::info!("Press Ctrl+C to shutdown the server...");

    tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => (),
        _ = server.handle() => (),
    }
    tracing::info!("Shutting down server...");
    server.shutdown().await
}
