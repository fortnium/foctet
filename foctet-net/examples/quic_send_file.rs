use clap::Parser;
use foctet_core::{
    content::{ContentId, TransferTicket},
    error::{ConnectionError, StreamError},
    frame::{FileMetadata, Frame, FrameType, Payload},
    node::{NodeAddr, NodeId},
};
use foctet_net::connection::{
    quic::{QuicConnection, QuicSocket},
    FoctetStream,
};
use foctet_net::config::EndpointConfig;
use tokio_util::sync::CancellationToken;
use std::path::PathBuf;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::OnceLock,
};
use tokio::sync::{mpsc, RwLock};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use anyhow::Result;

// Lazy static map to store file metadata
static METADATA_STORE: OnceLock<RwLock<HashMap<ContentId, FileMetadata>>> = OnceLock::new();
static LOCALPATH_STORE: OnceLock<RwLock<HashMap<ContentId, PathBuf>>> = OnceLock::new();

/// Command line arguments for the file sender.
#[derive(Parser, Debug)]
struct Args {
    /// Path of the file to send.
    #[arg(
        short = 'f',
        long = "file",
        help = "Path of the file to send.",
        required = true
    )]
    file_path: PathBuf,

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
async fn main() -> Result<()> {
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
    let mut socket_config = EndpointConfig::new()
        .with_max_read_buffer_size()
        .with_max_write_buffer_size()
        .with_insecure(false);

    if args.cert_path.is_some() & args.cert_path.is_some() {
        socket_config.cert_path = args.cert_path;
        socket_config.key_path = args.key_path;
    }

    let node_id = NodeId::generate();
    let node_addr = NodeAddr::new(node_id.clone()).with_socket_addresses(socket_config.server_addresses());

    let mut quic_socket = QuicSocket::new(node_id, socket_config)?;

    let (conn_tx, mut conn_rx) = mpsc::channel::<QuicConnection>(100);
    tracing::info!("Starting QUIC listener...");
    let cancel_token = CancellationToken::new();
    // Start the QUIC listener
    tokio::spawn(async move {
        match quic_socket.listen(conn_tx, cancel_token).await {
            Ok(_) => {
                tracing::info!("QUIC listener stopped.");
            }
            Err(e) => {
                tracing::error!("Error listening: {:?}", e);
            }
        }
    });

    let content_id = ContentId::new();

    for addr in &node_addr.socket_addresses {
        tracing::info!("QUIC listener listening on: {:?}", addr);
    }

    // Handle incoming connections
    tracing::info!("Waiting for incoming connections...");

    let file_metadata = foctet_core::fs::get_file_metadata(&args.file_path, false)?;

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

    while let Some(mut conn) = conn_rx.recv().await {
        tokio::spawn(async move {
            tracing::info!("New connection: {:?}", conn.remote_address());
            loop {
                tracing::info!("Waiting for incoming stream...");
                let mut stream = match conn.accept_stream().await {
                    Ok(stream) => stream,
                    Err(e) => {
                        if let Some(stream_error) = e.downcast_ref::<ConnectionError>() {
                            match stream_error {
                                ConnectionError::Closed => {
                                    tracing::info!(
                                        "Connection closed while waiting for {}",
                                        conn.next_stream_id
                                    );
                                }
                                _ => {
                                    tracing::error!("Error accepting stream: {:?}", e);
                                }
                            }
                        } else {
                            tracing::error!("Error accepting stream: {:?}", e);
                        }
                        break;
                    }
                };
                tokio::spawn(async move {
                    tracing::info!("New stream: {}", stream.stream_id);
                    loop {
                        match stream.receive_frame().await {
                            Ok(frame) => {
                                match frame.frame_type {
                                    FrameType::ContentRequest => {
                                        // 1. Check if the requested content
                                        let cid = if let Some(payload) = frame.payload {
                                            match payload {
                                                Payload::ContentId(cid) => cid,
                                                _ => {
                                                    tracing::error!(
                                                        "{} Missing content ID",
                                                        stream.stream_id
                                                    );
                                                    break;
                                                }
                                            }
                                        } else {
                                            tracing::error!("{} Missing payload", stream.stream_id);
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
                                                stream.stream_id,
                                                cid
                                            );
                                            break;
                                        };

                                        // 2. Send the file metadata
                                        let metadata_frame: Frame = Frame::builder()
                                            .with_fin(true)
                                            .with_frame_type(FrameType::TransferStart)
                                            .with_operation_id(stream.next_operation_id)
                                            .with_payload(Payload::FileMetadata(matadata))
                                            .build();
                                        tracing::info!("{} Sending metadata...", stream.stream_id);
                                        match stream.send_frame(metadata_frame).await {
                                            Ok(_) => {
                                                tracing::info!(
                                                    "{} Metadata sent.",
                                                    stream.stream_id
                                                );
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "{} Error sending metadata: {:?}",
                                                    stream.stream_id,
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
                                                stream.stream_id,
                                                cid
                                            );
                                            break;
                                        };
                                        let start_time = std::time::Instant::now();
                                        match stream.send_file(&file_path).await {
                                            Ok(_) => {
                                                tracing::info!("{} File sent.", stream.stream_id);
                                                let elapsed = start_time.elapsed();
                                                tracing::info!("Elapsed time: {:?}", elapsed);
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "{} Error sending file: {:?}",
                                                    stream.stream_id,
                                                    e
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    _ => {
                                        tracing::error!(
                                            "{} Unexpected frame type: {:?}",
                                            stream.stream_id,
                                            frame.frame_type
                                        );
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                if let Some(stream_error) = e.downcast_ref::<StreamError>() {
                                    match stream_error {
                                        StreamError::Closed => {
                                            tracing::info!("{} Stream closed.", stream.stream_id);
                                        }
                                        _ => {
                                            tracing::error!(
                                                "{} Error receiving frame: {:?}",
                                                stream.stream_id,
                                                e
                                            );
                                        }
                                    }
                                } else {
                                    tracing::error!(
                                        "{} Error receiving frame: {:?}",
                                        stream.stream_id,
                                        e
                                    );
                                }
                                break;
                            }
                        }
                    }
                    tracing::info!("{} Closing stream", stream.stream_id);
                    match stream.close().await {
                        Ok(_) => {
                            tracing::info!("{} Stream closed.", stream.stream_id);
                        }
                        Err(e) => match e.downcast_ref::<quinn::ClosedStream>() {
                            Some(_) => {
                                tracing::info!("{} Stream already closed.", stream.stream_id);
                            }
                            None => {
                                tracing::error!(
                                    "{} Error closing stream: {:?}",
                                    stream.stream_id,
                                    e
                                );
                            }
                        },
                    }
                });
            }
        });
    }
    Ok(())
}
