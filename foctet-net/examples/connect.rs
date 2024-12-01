use clap::Parser;
use foctet_core::frame::{Frame, FrameType};
use foctet_core::{frame::Payload, node::NodeId};
use foctet_net::connection::{quic::QuicSocket, FoctetStream};
use foctet_net::{socket::SocketConfig, tls::TlsConfig};
use std::net::SocketAddr;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use anyhow::Result;

/// Command line arguments.
#[derive(Parser, Debug)]
struct Args {
    /// Server address to connect to.
    //#[clap(default_value = "127.0.0.1:4432")]
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server address to connect to.",
        default_value = "127.0.0.1:4432"
    )]
    server_addr: SocketAddr,
    /// Server name to validate the certificate against.
    #[arg(
        short = 'n',
        long = "name",
        help = "Server name to validate the certificate against.",
        default_value = "localhost"
    )]
    server_name: String,
    /// Insecure mode to skip certificate verification.
    #[arg(short, long, help = "Insecure mode to skip certificate verification.")]
    insecure: bool,
}

#[tokio::main]
async fn main() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Args::parse();
    let tls_config = if args.insecure {
        TlsConfig::new_insecure_config().unwrap()
    } else {
        TlsConfig::new_native_config().unwrap()
    };
    let socket_config = SocketConfig::new(tls_config)
        .with_max_read_buffer_size()
        .with_max_write_buffer_size();

    let node_id = NodeId::generate();

}
