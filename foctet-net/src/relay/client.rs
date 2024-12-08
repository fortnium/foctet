use anyhow::Result;
use foctet_core::{frame::{Frame, FrameType, HandshakeData, Payload}, node::{NodeAddr, NodeId, RelayAddr, SessionId}};
use crate::{config::{EndpointConfig, TransportProtocol}, connection::{quic::QuicSocket, tcp::TcpSocket, FoctetStream, NetworkStream}};

pub struct RelayClient {
    pub node_addr: NodeAddr,
    pub config: EndpointConfig,
    /// QUIC socket
    quic_socket: QuicSocket,
    /// TCP socket
    tcp_socket: TcpSocket,
}

impl RelayClient {
    /// Create a new `RelayClient` with the given node address and configuration.
    pub fn new(node_addr: NodeAddr, config: EndpointConfig) -> Result<Self> {
        let quic_socket = QuicSocket::new(node_addr.node_id.clone(),  config.clone())?;
        let tcp_socket = TcpSocket::new(node_addr.node_id.clone(), config.clone())?;
        Ok(Self {
            node_addr,
            config,
            quic_socket: quic_socket,
            tcp_socket: tcp_socket,
        })
    }
    pub async fn connect(&mut self, node_id: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        match self.config.transport_protocol {
            TransportProtocol::Quic => self.connect_quic(node_id, relay_addr).await,
            TransportProtocol::Tcp => self.connect_tcp(node_id, relay_addr).await,
            TransportProtocol::Both => {
                match self.connect_quic(node_id.clone(), relay_addr.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(_) => {}
                }
                self.connect_tcp(node_id, relay_addr).await
            }
        }
    }
    pub async fn connect_quic(&mut self, node_id: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut conn = self.quic_socket.connect_relay(relay_addr).await?;
        let mut stream = conn.open_stream().await?;
        let frame: Frame = Frame::builder()
            .with_fin(true)
            .with_frame_type(FrameType::Handshake)
            .with_operation_id(stream.operation_id())
            .with_payload(Payload::handshake(HandshakeData::new(self.node_addr.node_id.clone(), node_id, SessionId::new())))
            .build();
        stream.send_frame(frame).await?;

        Ok(NetworkStream::Quic(stream))
    }
    pub async fn connect_tcp(&mut self, node_id: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut stream = self.tcp_socket.connect_relay(relay_addr).await?;
        let frame: Frame = Frame::builder()
            .with_fin(true)
            .with_frame_type(FrameType::Handshake)
            .with_operation_id(stream.operation_id())
            .with_payload(Payload::handshake(HandshakeData::new(self.node_addr.node_id.clone(), node_id, SessionId::new())))
            .build();
        stream.send_frame(frame).await?;
        Ok(NetworkStream::Tcp(stream))
    }
}
