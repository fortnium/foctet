use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;
use foctet_core::{content::TransferTicket, frame::{Frame, FrameType}, key::Keypair, metadata::FileMetadata};
use foctet_net::endpoint::Endpoint;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command line arguments for the file receiver.
#[derive(Parser, Debug)]
struct Args {
    /// The base32 transfer ticket to receive the file.
    #[arg(
        short = 't',
        long = "ticket",
        help = "The base32 transfer ticket to receive the file.",
        required = true
    )]
    ticket: String,
    /// Allow loopback address in the list of target socket addresses.
    #[arg(
        long = "loopback",
        help = "Allow loopback address in the list of target socket addresses.",
        default_value = "false"
    )]
    allow_loopback: bool,
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

    let keypair = Keypair::generate();

    tracing::info!("Node ID: {:?}", keypair.public().to_bytes());

    let mut endpoint = Endpoint::builder()
    .with_keypair(keypair)
    .with_quic()
    .with_tcp()
    .without_listen()
    .allow_loopback(args.allow_loopback)
    .with_max_read_buffer_size()
    .build()?;

    let mut conn = endpoint.connect_node(ticket.node_addr).await?;

    tracing::info!("Connected to: {:?}", conn.remote_addr());

    let mut stream = conn.open_stream().await?;

    tracing::info!("Stream opened ID: {:?}", stream.stream_id());

    // 1. Send a content request
    let content_request_frame: Frame = Frame::builder()
        .with_stream_id(stream.stream_id())
        .with_fin(true)
        .with_frame_type(FrameType::ContentRequest)
        .with_payload(ticket.content_id.to_bytes())
        .build();
    tracing::info!("Sending a content request...");
    tracing::debug!("Request: {:?}", content_request_frame);
    stream.send_frame(content_request_frame).await?;
    tracing::info!("Request sent.");

    // 2. Receive file metadata
    let frame = stream.receive_frame().await?;
    let file_metadata = match frame.header.frame_type {
        FrameType::FileMeta => {
            FileMetadata::from_bytes(&frame.payload)?
        },
        _ => return Err(anyhow::anyhow!("Invalid frame payload")),
    };
    tracing::info!("Received a content metadata");
    tracing::debug!("Metadata: {:?}", file_metadata);
    // 3. Receive the file content
    let save_path = if args.save_path.is_dir() {
        let mut save_path = args.save_path.clone();
        save_path.push(file_metadata.name);
        save_path
    } else {
        args.save_path.clone()
    };
    tracing::info!("Receiving file content...");
    let start_time = std::time::Instant::now();
    match stream.receive_file(&save_path, file_metadata.size).await {
        Ok(len) => {
            tracing::info!("File received successfully. Length: {:?}", len);
            tracing::info!("File saved to: {:?}", save_path);
            let elapsed = start_time.elapsed();
            tracing::info!("Elapsed time: {:?}", elapsed);
            // Calculate the Mbps
            let elapsed_secs = elapsed.as_secs_f64();
            let mbps = (file_metadata.size as f64 / 1_000_000.0) / elapsed_secs;
            tracing::info!("Speed: {:.2} Mbps", mbps);
        }
        Err(e) => {
            tracing::error!("Error receiving file: {:?}", e);
            return Err(e);
        }
    }

    stream.close().await?;
    tracing::info!("Stream closed: {:?}", stream.stream_id());
    conn.close().await?;
    tracing::info!("Connection closed: {:?}", conn.remote_addr());

    Ok(())
}
