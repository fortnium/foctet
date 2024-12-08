use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use foctet_core::node::{SessionId, NodeAddr, NodeId};
use tokio::sync::RwLock;

use crate::{config::EndpointConfig, connection::Session};

pub struct RelayServer {
    pub node_addr: NodeAddr,
    pub config: EndpointConfig,
    pub node_sessions: Arc<RwLock<HashMap<NodeId, Session>>>,
    pub relay_sessions: Arc<RwLock<HashMap<SessionId, HashSet<NodeId>>>>,
}
