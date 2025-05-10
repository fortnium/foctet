use std::path::PathBuf;
use bytes::Bytes;
use foctet_core::{frame::Frame, key::Keypair};
use foctet_net::{endpoint::Endpoint, transport::stream::Stream};
use stackaddr::StackAddr;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use anyhow::Result;

/// Command line arguments for the file sender.
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

    let mut endpoint = Endpoint::builder()
    .with_keypair(keypair)
    .with_addr(server_addr)?
    .allow_loopback(args.allow_loopback)
    .build()?;

    tracing::info!("Local node addr: {:?}", endpoint.node_addr());

    while let Some(mut conn) = endpoint.accept().await {
        tracing::info!("Accepted connection from {}", conn.remote_addr());
        tracing::info!("Remote Node ID: {}", conn.remote_node_id());
        tokio::spawn(async move {
            loop {
                // Accept new stream
                tracing::info!("Accepting a new stream...");
                match conn.accept_stream().await {
                    Ok(mut stream) => {
                        tokio::spawn(async move {
                            handle_stream(&mut stream).await;
                        });
                    }
                    Err(e) => {
                        tracing::info!("Error accepting stream: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
    Ok(())
}

async fn handle_stream(stream: &mut Stream) {
    tracing::info!("Handling stream ID: {}", stream.stream_id());
    loop {
        match stream.receive_frame().await {
            Ok(frame) => {
                tracing::info!("Received frame: {:?}", frame);
                // send a response
                let payload: Bytes = Bytes::from("Hello, client!");
                let response_frame: Frame = Frame::builder()
                    .with_stream_id(stream.stream_id())
                    .as_response()
                    .with_payload(payload)
                    .build();
                if let Err(e) = stream.send_frame(response_frame).await {
                    tracing::info!("Error sending frame: {:?}", e);
                    break;
                }
                tracing::info!("Sent response frame: {:?}", stream.stream_id());
            }
            Err(e) => {
                tracing::info!("Error receiving frame: {:?}", e);
                break;
            }
        }
    }
}
