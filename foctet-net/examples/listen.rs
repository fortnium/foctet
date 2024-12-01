use clap::Parser;
use foctet_core::{
    error::{ConnectionError, StreamError},
    frame::{Frame, FrameType, Payload},
    node::{NodeAddr, NodeId},
};
use foctet_net::{connection::{
    quic::{QuicConnection, QuicSocket},
    FoctetStream,
}, socket::Socket};
use foctet_net::{socket::SocketConfig, tls::TlsConfig};
use tokio_util::sync::CancellationToken;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use anyhow::Result;

/// Command line arguments for the file sender.
#[derive(Parser, Debug)]
struct Args {
    /// Server name
    #[arg(
        short = 'n',
        long = "name",
        help = "Server name",
        default_value = "localhost"
    )]
    server_name: String,
    /// Server address to bind to.
    //#[clap(default_value = "0.0.0.0:4432")]
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server address to bind to.",
        default_value = "0.0.0.0:4432"
    )]
    server_addr: SocketAddr,

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
async fn main() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse command line arguments
    let args = Args::parse();

    let tls_config = if args.cert_path.is_some() & args.cert_path.is_some() {
        TlsConfig::with_cert(&args.cert_path.unwrap(), &args.key_path.unwrap()).unwrap()
    } else {
        TlsConfig::new_insecure_config().unwrap()
    };
    let socket_config = SocketConfig::new(tls_config)
        .with_max_read_buffer_size()
        .with_max_write_buffer_size();

    let node_id = NodeId::generate();
    let node_addr = NodeAddr::new(node_id).with_socket_addr(args.server_addr).with_server_name(args.server_name);

    let socket = match Socket::spawn(node_addr, socket_config).await {
        Ok(socket) => socket,
        Err(e) => {
            tracing::error!("Failed to create socket: {:?}", e);
            return;
        }
    };

    

}
