use foctet::{core::node::{NodeId, NodePair}, net::connection::NetworkStream};
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

#[derive(Debug)]
pub struct RelayedConnection {
    pub src: NodeId,
    pub dst: NodeId,
    pub stream: NetworkStream,
}

pub enum ServerMessage {
    SendPacket(Packet),
    AddClient(RelayedConnection),
    RemoveClient(NodePair),
}
