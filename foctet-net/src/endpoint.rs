use crate::config::{EndpointConfig, ServerConfig, ClientConfig};
use anyhow::Result;
use anyhow::anyhow;
use foctet_core::node::NodeAddr;

pub mod endpoint_side {
    pub struct Client {

    }

    pub struct Server {

    }
}

pub struct Endpoint<Side> {
    side: Side,
    node_addr: NodeAddr,
    config: EndpointConfig,
}

/// The shared impl block for both client and server
impl<Side> Endpoint<Side> {
    
}

impl Endpoint<endpoint_side::Client> {
    /// Initialize as a client
    pub async fn new_client(node_addr: NodeAddr, client_config: ClientConfig) -> Self {
        Self {
            side: endpoint_side::Client {},
            node_addr,
            config: EndpointConfig::Client(client_config),
        }
    }
}

impl Endpoint<endpoint_side::Server> {
    /// Initialize as a server
    pub fn new_server(node_addr: NodeAddr, config: ServerConfig) -> Self {
        Self {
            side: endpoint_side::Server {},
            node_addr,
            config: EndpointConfig::Server(config),
        }
    }
}


