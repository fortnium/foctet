use std::{collections::HashMap, path::PathBuf, sync::OnceLock};
use foctet_core::{content::{ContentId, TransferTicket}, frame::{Frame, FrameType}, key::Keypair, metadata::FileMetadata};
use foctet_net::{endpoint::Endpoint, transport::stream::Stream};
use stackaddr::StackAddr;
use tokio::sync::RwLock;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use anyhow::Result;

// Lazy static map to store file metadata
static METADATA_STORE: OnceLock<RwLock<HashMap<ContentId, FileMetadata>>> = OnceLock::new();
static LOCALPATH_STORE: OnceLock<RwLock<HashMap<ContentId, PathBuf>>> = OnceLock::new();

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

    /// Path of the file to send.
    #[arg(
        short = 'f',
        long = "file",
        help = "Path of the file to send.",
        required = true
    )]
    file_path: PathBuf,
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
    .with_max_write_buffer_size()
    .build()?;

    tracing::info!("Local node addr: {:?}", endpoint.node_addr());

    let file_metadata = foctet_core::utils::fs::get_file_metadata(&args.file_path)?;

    let content_id = ContentId::new();

    // Register the file metadata
    METADATA_STORE
        .get_or_init(|| RwLock::new(HashMap::new()))
        .write()
        .await
        .insert(content_id.clone(), file_metadata);

    // Register the file path
    LOCALPATH_STORE
        .get_or_init(|| RwLock::new(HashMap::new()))
        .write()
        .await
        .insert(content_id.clone(), args.file_path.clone());

    let ticket = TransferTicket::new(endpoint.node_addr(), content_id);
    let ticket_base32 = ticket.to_base32()?;
    tracing::info!("Share this ticket with the receiver: {}", ticket_base32);

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
                match frame.header.frame_type {
                    FrameType::ContentRequest => {
                        // 1. Check the requested content
                        let cid = match ContentId::from_bytes(frame.payload) {
                            Ok(id) => id,
                            Err(e) => {
                                tracing::info!("Error decoding content ID: {:?}", e);
                                break;
                            }
                        };
                        let matadata = if let Some(metadata) = METADATA_STORE
                            .get_or_init(|| RwLock::new(HashMap::new()))
                            .read()
                            .await
                            .get(&cid)
                        {
                            metadata.clone()
                        } else {
                            tracing::error!(
                                "{} Content not found: {:?}",
                                stream.stream_id(),
                                cid
                            );
                            break;
                        };
                        let file_size: usize = matadata.size;
                        // 2. Send the file metadata
                        let payload = matadata.to_bytes()
                            .expect("Failed to serialize file metadata");
                        let metadata_frame: Frame = Frame::builder()
                            .with_stream_id(stream.stream_id())
                            .as_response()
                            .with_frame_type(FrameType::FileMeta)
                            .with_payload(payload)
                            .with_fin(true)
                            .build();

                        tracing::info!("{} Sending metadata...", stream.stream_id());
                        if let Err(e) = stream.send_frame(metadata_frame).await {
                            tracing::info!("Error sending frame: {:?}", e);
                            break;
                        }
                        // 3. Send the file in chunks
                        let file_path = if let Some(file_path) = LOCALPATH_STORE
                            .get_or_init(|| RwLock::new(HashMap::new()))
                            .read()
                            .await
                            .get(&cid)
                        {
                            file_path.clone()
                        } else {
                            tracing::error!(
                                "{} File path not found: {:?}",
                                stream.stream_id(),
                                cid
                            );
                            break;
                        };
                        let start_time = std::time::Instant::now();
                        match stream.send_file(&file_path).await {
                            Ok(_) => {
                                tracing::info!("{} File sent.", stream.stream_id());
                                let elapsed = start_time.elapsed();
                                tracing::info!("Elapsed time: {:?}", elapsed);
                                // Calculate the Mbps
                                let elapsed_secs = elapsed.as_secs_f64();
                                let mbps = (file_size as f64 / 1_000_000.0) / elapsed_secs;
                                tracing::info!("Speed: {:.2} Mbps", mbps);
                            }
                            Err(e) => {
                                tracing::error!(
                                    "{} Error sending file: {:?}",
                                    stream.stream_id(),
                                    e
                                );
                                break;
                            }
                        }
                    }
                    _ => {
                        tracing::info!("Received unexpected frame: {:?}", frame);
                    }
                }
            }
            Err(e) => {
                tracing::info!("Error receiving frame: {:?}", e);
                break;
            }
        }
    }
}
