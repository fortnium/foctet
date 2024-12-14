use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::Parser;
use anyhow::Result;
use foctet_core::frame::{Frame, FrameType, Payload};
use foctet_core::node::{NodeAddr, NodeId};
use foctet_net::{connection::FoctetStream, endpoint::Endpoint};
use tokio::sync::mpsc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command line arguments for the file receiver.
#[derive(Parser, Debug)]
struct Args {
    /// Server Node address to connect to.
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server Node address to connect to.",
    )]
    server_node_addr: String,
    /// Insecure mode to skip certificate verification.
    #[arg(short, long, help = "Insecure mode to skip certificate verification.")]
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
    let args = Args::parse();

    let dummy_server_addr : SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433);
    let node_id = NodeId::generate();
    let node_addr = NodeAddr::new(node_id)
    .with_server_name("localhost".to_string())
    .with_socket_addr(dummy_server_addr);

    let server_node_addr: NodeAddr = NodeAddr::from_base32(&args.server_node_addr)?;
    
    // Create a new endpoint
    let mut endpoint = Endpoint::builder()
        .with_node_addr(node_addr)
        .with_insecure(args.insecure)
        .with_server_addr(dummy_server_addr)
        .with_include_loopback(true)
        .build()
        .await?;

    // Connect to the server
    tracing::info!("Connecting to the server: {:?}", server_node_addr);
    let mut stream = endpoint.connect(server_node_addr).await?;

    tracing::info!("Connected to the server: {:?}", stream.remote_address());

    // Create an mpsc channel for communication
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Spawn a task to handle sending messages from standard input
    tokio::spawn(async move {
        let mut buffer = String::new();
        let stdin = std::io::stdin();
        while let Ok(_size) = stdin.read_line(&mut buffer) {
            let message = buffer.clone();
            if message.trim().eq_ignore_ascii_case("exit") {
                // Send a termination signal via the channel
                if let Err(_) = tx.send("exit".to_string()).await {
                    break;
                }
                break;
            }
            if tx.send(message).await.is_err() {
                break;
            }
            buffer.clear();
        }
    });

    // Main loop to handle receiving and sending messages
    loop {
        tokio::select! {
            Some(line) = rx.recv() => {
                if line.trim().eq_ignore_ascii_case("exit") {
                    tracing::info!("Exit signal received. Closing connection.");
                    stream.close().await?;
                    break;
                }
                // Build and send a frame with the user's input
                let frame = Frame::builder()
                    .with_frame_type(FrameType::Text)
                    .with_payload(Payload::Text(line))
                    .as_request()
                    .build();

                if let Err(e) = stream.send_frame(frame).await {
                    tracing::error!("Failed to send message: {:?}", e);
                    break;
                }
            }
            Ok(frame) = stream.receive_frame() => {
                // Handle received frame
                if let Some(Payload::Text(message)) = frame.payload {
                    println!("[Server]: {}", message);
                } else {
                    tracing::warn!("Received unexpected frame: {:?}", frame);
                }
            }
            else => {
                tracing::info!("Connection closed.");
                break;
            }
        }
    }

    Ok(())
}
