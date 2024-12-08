use clap::Parser;
use foctet_core::frame::{Frame, FrameType};
use foctet_core::{frame::Payload, node::NodeId};
use foctet_net::connection::{quic::QuicSocket, FoctetStream};
use foctet_net::config::EndpointConfig;
use std::net::SocketAddr;
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
    let args = Args::parse();
    let socket_config = EndpointConfig::new()
        .with_max_read_buffer_size()
        .with_max_write_buffer_size()
        .with_insecure(args.insecure);

    let node_id = NodeId::generate();

    let mut quic_socket = QuicSocket::new_client(node_id.clone(), socket_config)?;
    match quic_socket
        .connect(args.server_addr, &args.server_name)
        .await
    {
        Ok(mut conn) => {
            // Connection
            tracing::info!("Connected to: {:?}", conn.remote_address());
            {
                // Stream
                let mut stream = conn.open_stream().await?;
                let frame: Frame = Frame::builder()
                    .with_fin(true)
                    .with_frame_type(FrameType::Text)
                    .with_operation_id(stream.next_operation_id)
                    .with_payload(Payload::text("Hello! from client.".to_string()))
                    .build();
                tracing::info!("Sending a message...");
                tracing::info!("Frame: {:?}", frame);
                stream.send_frame(frame).await?;
                tracing::info!("Message sent.");
                // Wait for the server to respond
                let frame = stream.receive_frame().await?;
                tracing::info!("Received a message.");
                tracing::info!("Frame: {:?}", frame);
                tracing::info!("closing stream...");
                stream.close().await?;
                tracing::info!("stream closed.");
            }
            tracing::info!("closing connection...");
            conn.close().await?;
            tracing::info!("connection closed.");
        }
        Err(e) => {
            tracing::error!("Error connecting: {:?}", e);
        }
    }
    Ok(())
}
