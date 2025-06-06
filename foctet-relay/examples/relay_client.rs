use clap::Parser;
use anyhow::Result;
use foctet_core::{addr::node::{NodeAddr, RelayAddr}, frame::Frame, id::NodeId, key::Keypair, transport::TransportKind};
use foctet_net::{config::TransportConfig, device::get_default_stack_addrs};
use foctet_relay::client::RelayClient;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command line arguments for the relay client.
#[derive(Parser, Debug)]
struct Args {
    /// The base32 relay server address to connect to.
    #[arg(
        short = 'r',
        long = "relay",
        help = "The base32 relay server address to connect to.",
    )]
    relay_addr: String,
    /// The base32 node ID to connect to.
    #[arg(
        short = 'n',
        long = "node",
        help = "The base32 node ID to connect to.",
    )]
    node_id: Option<String>,
    /// The transport protocol to use for the connection.
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

    let relay_addr_base32: String = args.relay_addr;
    let relay_addr: RelayAddr = RelayAddr::from_base32(&relay_addr_base32)?;
    
    let keypair = Keypair::generate();

    let local_node_addr = NodeAddr {
        node_id: keypair.public(),
        addresses: get_default_stack_addrs(&[TransportKind::Quic], false).into_iter().collect(),
        relay_addr: Some(relay_addr.clone()),
    };

    tracing::info!("Node ID: {:?}", keypair.public().to_base32());

    let config = TransportConfig::new(keypair).unwrap();

    let mut client = RelayClient::builder()
        .with_config(config)
        .with_local_node_addr(local_node_addr)
        .with_protocol(TransportKind::Quic)
        .with_relay_addr(relay_addr)
        .with_allow_loopback(args.allow_loopback)
        .build()?;

    if let Some(node_id_base32) = args.node_id {
        tracing::info!("Connecting to node ID: {}", node_id_base32);
        let node_id = NodeId::from_base32(&node_id_base32)?;

        let mut stream = client.open_stream(node_id).await?;
        tracing::info!("Connected to node ID: {:?}", node_id.to_bytes());
        let frame = Frame::builder()
        .with_stream_id(stream.stream_id())
        .as_request()
        .with_payload("Hello, acceptor node!".into())
        .build();
        stream.send_frame(frame).await?;
        tracing::info!("Message sent to acceptor");
        let response = stream.receive_frame().await?;
        tracing::info!("Received response: {:?}", response.payload);

        // wait for 1 second before closing the stream
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        stream.close().await?;
        tracing::info!("Stream closed");
    } else {
        tracing::info!("Connected to relay server. Waiting for incoming stream...");
        while let Some(mut stream) = client.accept_stream().await {
            
            tracing::info!("Accepted incoming stream: {:?}", stream.stream_id());
            
            let response = stream.receive_frame().await?;
            tracing::info!("Received frame: {:?}", response.payload);

            tracing::info!("Sending response back ...");
            let frame = Frame::builder()
                .with_stream_id(stream.stream_id())
                .as_request()
                .with_payload("Hello, connector node!".into())
                .build();
            stream.send_frame(frame).await?;
            tracing::info!("Message sent to connector");

            // wait for 1 second before closing the stream
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            stream.close().await?;
            tracing::info!("Stream closed");
        }
    }
    tracing::info!("Shutting down the relay client...");
    client.shutdown().await?;
    tracing::info!("Relay client closed");
    Ok(())
}
