use foctet_core::{addr::node::NodeAddr, id::NodeId};
use foctet_net::transport::{connection::Connection, stream::Stream};

pub struct RelayClient {
    pub node_id: NodeId,
    pub node_addr: NodeAddr,
    pub conn: Connection,
    pub control_stream: Option<Stream>,
}
