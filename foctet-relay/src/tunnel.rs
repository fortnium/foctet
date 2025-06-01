use foctet_core::id::NodeId;
use foctet_net::transport::stream::Stream;

pub struct Tunnel {
    pub src_node: NodeId,
    pub dst_node: NodeId,
    pub src_stream: Stream,
    pub dst_stream: Stream,
}

pub struct TunnelInfo {
    pub src_node: NodeId,
    pub dst_node: NodeId,
    pub incoming_traffic: u64,
    pub outgoing_traffic: u64,
}

impl TunnelInfo {
    pub fn new(src_node: NodeId, dst_node: NodeId) -> Self {
        Self {
            src_node,
            dst_node,
            incoming_traffic: 0,
            outgoing_traffic: 0,
        }
    }

    pub fn update_incoming_traffic(&mut self, bytes: u64) {
        self.incoming_traffic += bytes;
    }

    pub fn update_outgoing_traffic(&mut self, bytes: u64) {
        self.outgoing_traffic += bytes;
    }
}
