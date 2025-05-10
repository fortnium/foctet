use bytes::Bytes;
use clap::Parser;
use anyhow::Result;
use foctet_core::{frame::Frame, key::Keypair};
use foctet_net::endpoint::Endpoint;
use stackaddr::StackAddr;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command line arguments for the file receiver.
#[derive(Parser, Debug)]
struct Args {
    /// Server stack address to connect to.
    #[arg(
        short = 'a',
        long = "addr",
        help = "Server stack address to connect to.",
    )]
    server_stack_addr: String,
    /// Allow loopback address in the list of target socket addresses.
    #[arg(
        long = "loopback",
        help = "Allow loopback address in the list of target socket addresses.",
        default_value = "false"
    )]
    allow_loopback: bool,
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
    .with_quic()
    .with_tcp()
    .without_listen()
    .allow_loopback(args.allow_loopback)
    .build()?;

    let mut conn = endpoint.connect(server_addr).await?;

    tracing::info!("Connected to: {:?}", conn.remote_addr());

    let mut stream = conn.open_stream().await?;

    tracing::info!("Stream opened ID: {:?}", stream.stream_id());

    // Send a message to the server
    let payload: Bytes = Bytes::from("Hello, server!");
    let frame: Frame = Frame::builder()
        .with_stream_id(stream.stream_id())
        .as_request()
        .with_payload(payload)
        .build();

    stream.send_frame(frame).await?;

    tracing::info!("Sent frame: {:?}", stream.stream_id());
    // Receive a message from the server
    let res_frame = stream.receive_frame().await?;
    tracing::info!("Received frame: {:?}", res_frame);

    stream.close().await?;
    tracing::info!("Stream closed: {:?}", stream.stream_id());
    conn.close().await?;
    tracing::info!("Connection closed: {:?}", conn.remote_addr());

    Ok(())
}
