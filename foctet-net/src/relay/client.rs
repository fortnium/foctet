use anyhow::Result;
use foctet_core::node::{NodeAddr, NodeId, RelayAddr};
use crate::{config::{EndpointConfig, TransportProtocol}, connection::{quic::QuicSocket, tcp::TcpSocket, FoctetStream, NetworkStream}};

#[derive(Clone)]
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
        let quic_socket = QuicSocket::new_client(node_addr.node_id.clone(), config.clone())?;
        let tcp_socket = TcpSocket::new(node_addr.node_id.clone(), config.clone())?;
        Ok(Self {
            node_addr,
            config,
            quic_socket: quic_socket,
            tcp_socket: tcp_socket,
        })
    }
    pub async fn open_control_stream(&mut self) -> Result<NetworkStream> {
        let relay_addr = if let Some(relay_addr) = &self.node_addr.relay_addr {
            relay_addr.clone() 
        } else{
            return Err(anyhow::anyhow!("No relay address found"));
        };
        match self.config.transport_protocol {
            TransportProtocol::Quic => self.open_control_stream_quic(relay_addr.clone()).await,
            TransportProtocol::Tcp => self.open_control_stream_tcp(relay_addr.clone()).await,
            TransportProtocol::Both => {
                match self.open_control_stream_quic(relay_addr.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(_) => {
                        self.open_control_stream_tcp(relay_addr.clone()).await
                    }
                }
            }
        }
    }
    pub async fn open_control_stream_quic(&mut self, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut conn = self.quic_socket.connect_relay(relay_addr).await?;
        let mut stream = conn.open_stream().await?;
        stream.handshake(NodeId::zero(),None).await?;
        Ok(NetworkStream::Quic(stream))
    }
    pub async fn open_control_stream_tcp(&mut self, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut stream = self.tcp_socket.connect_relay(relay_addr).await?;
        stream.handshake(NodeId::zero(),None).await?;
        Ok(NetworkStream::Tcp(stream))
    }
    pub async fn connect(&mut self, dst_node_addr: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        match self.config.transport_protocol {
            TransportProtocol::Quic => self.connect_quic(dst_node_addr, relay_addr).await,
            TransportProtocol::Tcp => self.connect_tcp(dst_node_addr, relay_addr).await,
            TransportProtocol::Both => {
                match self.connect_quic(dst_node_addr.clone(), relay_addr.clone()).await {
                    Ok(stream) => return Ok(stream),
                    Err(_) => {}
                }
                self.connect_tcp(dst_node_addr, relay_addr).await
            }
        }
    }
    pub async fn connect_quic(&mut self, dst_node_addr: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut conn = self.quic_socket.connect_relay(relay_addr).await?;
        let mut stream = conn.open_stream().await?;
        stream.handshake(dst_node_addr, None).await?;
        Ok(NetworkStream::Quic(stream))
    }
    pub async fn connect_tcp(&mut self, dst_node_addr: NodeId, relay_addr: RelayAddr) -> Result<NetworkStream> {
        let mut stream = self.tcp_socket.connect_relay(relay_addr).await?;
        stream.handshake(dst_node_addr, None).await?;
        Ok(NetworkStream::Tcp(stream))
    }
}
