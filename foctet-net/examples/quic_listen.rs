use clap::Parser;
use foctet_core::{
    error::{ConnectionError, StreamError},
    frame::{Frame, FrameType, Payload},
    node::NodeId,
};
use foctet_net::connection::{
    quic::{QuicConnection, QuicSocket},
    FoctetStream,
};
use foctet_net::config::EndpointConfig;
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

    /// Insecure mode to use self-signed certificate and skip certificate verification.
    #[arg(short, long, help = "Insecure mode to use self-signed certificate and skip certificate verification.")]
    insecure: bool,
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
        .with_insecure(args.insecure);

    if args.cert_path.is_some() & args.cert_path.is_some() {
        socket_config.cert_path = args.cert_path;
        socket_config.key_path = args.key_path;
    }

    let node_id = NodeId::generate();

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
    tracing::info!("QUIC listener listening on: {:?}", args.server_addr);
    // Handle incoming connections
    tracing::info!("Waiting for incoming connections...");
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
                                if frame.payload_len() < 128 {
                                    tracing::info!(
                                        "{} Received frame: {:?}",
                                        stream.stream_id,
                                        frame
                                    );
                                } else {
                                    tracing::info!(
                                        "{} Received frame type: {:?}",
                                        stream.stream_id,
                                        frame.frame_type
                                    );
                                }
                                tracing::info!(
                                    "{} Total length: {:?}",
                                    stream.stream_id,
                                    frame.len()
                                );
                                tracing::info!(
                                    "{} Payload length: {}",
                                    stream.stream_id,
                                    frame.payload_len()
                                );
                                // Send a response
                                let frame: Frame = Frame::builder()
                                    .with_fin(true)
                                    .with_frame_type(FrameType::Text)
                                    .with_operation_id(stream.next_operation_id)
                                    .with_payload(Payload::text("Hello! from server.".to_string()))
                                    .build();
                                tracing::info!("{} Sending a response frame...", stream.stream_id);
                                tracing::info!("Frame: {:?}", frame);
                                match stream.send_frame(frame).await {
                                    Ok(_) => {
                                        tracing::info!("{} Response sent.", stream.stream_id);
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "{} Error sending response: {:?}",
                                            stream.stream_id,
                                            e
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
                });
            }
        });
    }
    Ok(())
}
