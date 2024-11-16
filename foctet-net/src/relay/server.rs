use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use foctet_core::node::{ConnectionId, NodeAddr, NodeId};
use tokio::sync::RwLock;

use crate::{socket::SocketConfig, connection::Session};

pub struct RelayServer {
    pub node_addr: NodeAddr,
    pub config: SocketConfig,
    pub node_sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    pub relay_sessions: Arc<RwLock<HashMap<ConnectionId, HashSet<NodeId>>>>,
}
