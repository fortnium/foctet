use std::path::PathBuf;
use foctet_core::key::Keypair;
use foctet_net::{config::TransportConfig};
use foctet_relay::server::{RelayServer, ServerEvent};
use stackaddr::StackAddr;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use anyhow::Result;

/// Command line arguments for the relay server.
#[derive(Parser, Debug)]
struct Args {
    /// Server stack address to bind to.
    //#[clap(default_value = "/ip4/0.0.0.0/quic/4432")]
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server stack address to bind to.",
        default_value = "/ip4/0.0.0.0/udp/4432/quic"
    )]
    server_stack_addr: String,

    /// The server name for subject-alt-names (SANs) in the certificate. 
    #[arg(
        short = 'n',
        long = "name",
        help = "The server name for subject-alt-names (SANs) in the certificate. ",
        default_value = "localhost"
    )]
    server_name: String,

    /// Allow loopback address in the list of listening socket addresses.
    #[arg(
        long = "loopback",
        help = "Allow loopback address in the list of listening socket addresses.",
        default_value = "false"
    )]
    allow_loopback: bool,

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

    let server_addr = args.server_stack_addr.parse::<StackAddr>()?;

    let keypair = Keypair::generate();

    tracing::info!("Node ID: {:?}", keypair.public().to_bytes());

    let config = TransportConfig::new(keypair).unwrap();

    // Create and spawn the relay server
    let mut server = RelayServer::builder()
        .with_config(config)
        .with_addrs(&[server_addr])
        .with_allow_loopback(args.allow_loopback)
        .build()?;

    println!("Relay server listening on: {:?}", server.addrs());
    println!("Relay Address base32: {:?}", server.node_addr().to_base32()?);

    // Listening envents...
    while let Some(event) = server.next_event().await {
        match event {
            ServerEvent::ClientConnected(client_info) => {
                tracing::info!("Client connected: {:?}", client_info);
            }
            ServerEvent::ClientDisconnected(node_id) => {
                tracing::info!("Client disconnected: {:?}", node_id);
            }
            ServerEvent::ClientRegistered(client_info) => {
                tracing::info!("Client registered: {:?}", client_info);
            }
            ServerEvent::ClientUnregistered(node_id) => {
                tracing::info!("Client unregistered: {:?}", node_id);
            }
            ServerEvent::TunnelEstablished { src_node, dst_node } => {
                tracing::info!("Tunnel established from {} to {}", src_node, dst_node);
            }
            ServerEvent::TunnelFailed { src_node, dst_node, reason } => {
                tracing::error!("Tunnel failed from {} to {}: {}", src_node, dst_node, reason);
            }
        }
    }
    Ok(())
}
