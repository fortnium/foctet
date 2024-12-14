use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;
use anyhow::Result;
use foctet_core::content::TransferTicket;
use foctet_core::frame::{Frame, FrameType, Payload};
use foctet_core::node::{NodeAddr, NodeId};
use foctet_net::{connection::FoctetStream, endpoint::Endpoint};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command line arguments for the file receiver.
#[derive(Parser, Debug)]
struct Args {
    /// The transfer ticket to receive the file.
    //#[clap(default_value = "127.0.0.1:4432")]
    #[arg(
        short = 't',
        long = "ticket",
        help = "The transfer ticket to receive the file.",
        required = true
    )]
    ticket: String,
    /// Insecure mode to skip certificate verification.
    #[arg(short, long, help = "Insecure mode to skip certificate verification.")]
    insecure: bool,
    /// Directory Path where the received file should be saved.
    #[arg(
        short = 's',
        long = "save",
        help = "Directory path where the received file should be saved.",
        required = true
    )]
    save_path: PathBuf,
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

    let ticket_base32: String = args.ticket;
    let ticket: TransferTicket = TransferTicket::from_base32(&ticket_base32)?;

    let dummy_server_addr : SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433);
    let node_id = NodeId::generate();
    let node_addr = NodeAddr::new(node_id)
    .with_server_name("localhost".to_string())
    .with_socket_addr(dummy_server_addr);

    let server_node_addr: NodeAddr = ticket.node_addr;
    
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

    // Stream
    // 1. Send a content request
    let content_request_frame: Frame = Frame::builder()
        .with_fin(true)
        .with_frame_type(FrameType::ContentRequest)
        .with_operation_id(stream.operation_id())
        .with_payload(Payload::ContentId(ticket.content_id))
        .build();
    tracing::info!("Sending a content request...");
    tracing::debug!("Request: {:?}", content_request_frame);
    stream.send_frame(content_request_frame).await?;
    tracing::info!("Request sent.");
    // 2. Wait for the server to respond with a transfer start frame including the metadata
    let metadata_frame = stream.receive_frame().await?;
    if metadata_frame.frame_type != FrameType::TransferStart {
        tracing::error!(
            "Expected a transfer start frame, but received: {:?}",
            metadata_frame
        );
        return Err(anyhow::anyhow!(
            "Expected a transfer start frame, but received: {:?}",
            metadata_frame
        ));
    }
    let metadata = if let Some(payload) = &metadata_frame.payload {
        match payload {
            Payload::FileMetadata(metadata) => metadata.clone(),
            _ => {
                tracing::error!(
                    "Expected a content metadata, but received: {:?}",
                    metadata_frame
                );
                return Err(anyhow::anyhow!(
                    "Expected a content metadata, but received: {:?}",
                    metadata_frame
                ));
            }
        }
    } else {
        tracing::error!(
            "Expected a content metadata, but received: {:?}",
            metadata_frame
        );
        return Err(anyhow::anyhow!(
            "Expected a content metadata, but received: {:?}",
            metadata_frame
        ));
    };
    tracing::info!("Received a content metadata");
    tracing::debug!("Metadata: {:?}", metadata);
    // 3. Receive the file content
    let save_path = if args.save_path.is_dir() {
        let mut save_path = args.save_path.clone();
        save_path.push(metadata.name);
        save_path
    } else {
        args.save_path.clone()
    };
    tracing::info!("Receiving file content...");
    let start_time = std::time::Instant::now();
    match stream.receive_file(&save_path).await {
        Ok(_) => {
            tracing::info!("File content received.");
            tracing::info!("File saved to: {:?}", save_path);
            let elapsed = start_time.elapsed();
            tracing::info!("Elapsed time: {:?}", elapsed);
        }
        Err(e) => {
            tracing::error!("Failed to receive file content: {:?}", e);
            return Err(e);
        }
    }
    tracing::info!("closing stream...");
    stream.close().await?;
    tracing::info!("stream closed.");

    Ok(())
}
