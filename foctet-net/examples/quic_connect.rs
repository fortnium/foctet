use clap::Parser;
use foctet_core::{frame::Payload, node::NodeId};
use foctet_net::connection::{quic::QuicSocket, FoctetStream};
use foctet_net::{config::SocketConfig, tls::TlsConfig};
use std::net::SocketAddr;
use foctet_core::frame::{Frame, FrameHeader, FrameType};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use anyhow::Result;

/// Command line arguments for the file receiver.
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
    // Parse command line arguments
    let args = Args::parse();
    let tls_config = if args.insecure {
        TlsConfig::new_insecure_config()?
    } else {
        TlsConfig::new_native_config()?
    };
    let socket_config = SocketConfig::new(tls_config)
        .with_max_read_buffer_size()
        .with_max_write_buffer_size();

    let node_id = NodeId::generate();

    let mut quic_socket = QuicSocket::new_client(node_id, socket_config)?;
    match quic_socket.connect(args.server_addr, &args.server_name).await {
        Ok(conn) => {
            let mut conn = conn.lock().await;
            tracing::info!("Connected to: {:?}", conn.node_id);
            let stream = conn.open_stream().await?;
            let mut stream = stream.lock().await;
            let node_id = NodeId::generate();
            let frame_header = FrameHeader::builder()
                .with_frame_type(FrameType::Message)
                .with_node_id(node_id)
                .with_stream_id(stream.stream_id)
                .build();
            let payload = Payload::Text("Hello, world!".to_string());
            let frame = Frame::new(frame_header, Some(payload));
            tracing::info!("Sending a message...");
            tracing::info!("Frame: {:?}", frame);
            stream.send_frame(frame).await?;
            tracing::info!("Message sent.");
            stream.close().await?;
            conn.close().await?;
        }
        Err(e) => {
            tracing::error!("Error connecting: {:?}", e);
        }
    }
    Ok(())
}
