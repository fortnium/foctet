use foctet::{core::node::NodeId, net::connection::NetworkStream};
use bytes::Bytes;

#[derive(Debug)]
pub struct Packet {
    pub src: NodeId,
    pub dst: NodeId,
    pub data: Bytes,
}

impl Packet {
    pub fn new(src: NodeId, dst: NodeId, data: Bytes) -> Self {
        Self { src, dst, data }
    }
}

pub enum ServerMessage {
    SendPacket(Packet),
    AddClient(NodeId, NetworkStream),
    RemoveClient(NodeId),
}
