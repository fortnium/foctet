use serde::{Deserialize, Serialize};
use foctet_core::{addr::node::NodeAddr, id::NodeId};
use stackaddr::StackAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayServerConfig {
    pub listen_addrs: Vec<StackAddr>,
    pub max_clients: Option<usize>,
    pub auth_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayClientConfig {
    pub relay_addrs: Vec<StackAddr>,
    pub node_id: NodeId,
    pub node_addr: NodeAddr,
    pub register_timeout_secs: u64,
    pub tunnel_timeout_secs: u64,
}
