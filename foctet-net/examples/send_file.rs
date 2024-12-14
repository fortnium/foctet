use std::{collections::{BTreeSet, HashMap}, net::SocketAddr, path::PathBuf, sync::OnceLock};
use foctet_core::{content::{ContentId, TransferTicket}, frame::{FileMetadata, Frame, FrameType, Payload}, node::{NodeAddr, NodeId}};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use anyhow::Result;
use tokio::sync::RwLock;
use foctet_net::{config::TransportProtocol, connection::{FoctetStream, NetworkStream}, endpoint::Endpoint};

// Lazy static map to store file metadata
static METADATA_STORE: OnceLock<RwLock<HashMap<ContentId, FileMetadata>>> = OnceLock::new();
static LOCALPATH_STORE: OnceLock<RwLock<HashMap<ContentId, PathBuf>>> = OnceLock::new();

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

    let mut server_addrs: BTreeSet<SocketAddr> = BTreeSet::new();
    if args.server_addr == foctet_core::default::DEFAULT_SERVER_V4_ADDR {
        server_addrs = foctet_net::device::get_default_server_addrs(true);
    }else{
        server_addrs.insert(args.server_addr);
    }

    let node_id = NodeId::generate();
    let node_addr = NodeAddr::new(node_id)
    .with_server_name(args.server_name.clone())
    .with_socket_addresses(server_addrs);

    // Create a new server endpoint
    let mut endpoint = Endpoint::builder()
        .with_node_addr(node_addr.clone())
        .with_server_addr(args.server_addr)
        .with_subject_alt_name(args.server_name)
        .with_cert_path_option(args.cert_path)
        .with_key_path_option(args.key_path)
        .with_insecure(args.insecure)
        .build().await?;

    tracing::info!("Server listening on {}", args.server_addr);
    tracing::info!("NodeAddr {:?}", endpoint.node_addr);
    tracing::info!("Node address for connect: {}", endpoint.node_addr.to_base32()?);

    // Start listening for incoming connections
    let mut listener = endpoint.listen().await?;
    tracing::info!("Waiting for incoming connections...");

    let file_metadata = foctet_core::fs::get_file_metadata(&args.file_path, false)?;

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

    let ticket = TransferTicket::new(node_addr, content_id);
    let ticket_base32 = ticket.to_base32()?;
    tracing::info!("Share this ticket with the receiver: {}", ticket_base32);

    // Handle incoming streams
    while let Some(mut stream) = listener.accept().await {
        tokio::spawn(async move {
            match stream.transport_protocol() {
                TransportProtocol::Quic => {
                    tracing::info!("New QUIC connection from: {}", stream.remote_address());
                }
                TransportProtocol::Tcp => {
                    tracing::info!("New TCP connection from: {}", stream.remote_address());
                }
                _ => {
                    tracing::info!("New connection from: {}", stream.remote_address());
                }
            }
            handle_stream(&mut stream).await;
        });
    }
    Ok(())
}

async fn handle_stream(stream: &mut NetworkStream) {
    tracing::info!("Handling stream from: {}", stream.remote_address());
    loop {
        match stream.receive_frame().await {
            Ok(frame) => {
                tracing::info!("Received frame: {:?}", frame);
                match frame.frame_type {
                    FrameType::Connect => {
                        let frame: Frame = Frame::builder()
                            .with_fin(true)
                            .with_frame_type(FrameType::Connected)
                            .as_response()
                            .build();
                        if let Err(e) = stream.send_frame(frame).await {
                            tracing::info!("Failed to send frame back: {:?}", e);
                        }
                    }
                    FrameType::ContentRequest => {
                        // 1. Check if the requested content
                        let cid = if let Some(payload) = frame.payload {
                            match payload {
                                Payload::ContentId(cid) => cid,
                                _ => {
                                    tracing::error!(
                                        "{} Missing content ID",
                                        stream.stream_id()
                                    );
                                    break;
                                }
                            }
                        } else {
                            tracing::error!("{} Missing payload", stream.stream_id());
                            break;
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

                        // 2. Send the file metadata
                        let metadata_frame: Frame = Frame::builder()
                            .with_fin(true)
                            .with_frame_type(FrameType::TransferStart)
                            .with_operation_id(stream.operation_id())
                            .with_payload(Payload::FileMetadata(matadata))
                            .build();
                        tracing::info!("{} Sending metadata...", stream.stream_id());
                        match stream.send_frame(metadata_frame).await {
                            Ok(_) => {
                                tracing::info!(
                                    "{} Metadata sent.",
                                    stream.stream_id()
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "{} Error sending metadata: {:?}",
                                    stream.stream_id(),
                                    e
                                );
                                break;
                            }
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
                        let message = format!("[{}]Hello! from server.", frame.operation_id);
                        let res_frame = Frame::builder()
                        .with_frame_type(FrameType::Text)
                        .with_fin(true)
                        .as_response()
                        .with_payload(Payload::Text(message))
                        .build();
                        if let Err(e) = stream.send_frame(res_frame).await {
                            tracing::info!("Failed to send frame back: {:?}", e);
                        }
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

/* /// Handle an individual stream with async Read/Write
async fn handle_stream<T>(stream: &mut T)
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = [0u8; 1024];

    // Read data from the stream
    match stream.read(&mut buffer).await {
        Ok(0) => {
            tracing::info!("Connection closed by client.");
        }
        Ok(n) => {
            tracing::info!("Received {} bytes: {:?}", n, &buffer[..n]);

            // Echo back the received data
            if let Err(e) = stream.write_all(&buffer[..n]).await {
                tracing::info!("Failed to send data back: {:?}", e);
            }
        }
        Err(e) => {
            tracing::info!("Error reading from stream: {:?}", e);
        }
    }
} */
