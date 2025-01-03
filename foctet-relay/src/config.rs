use foctet::core::default::{DEFAULT_RELAY_PACKET_QUEUE_CAPACITY, DEFAULT_RELAY_SERVER_CHANNEL_CAPACITY};
use foctet::core::node::{NodeAddr, NodeId, RelayAddr};
use foctet::net::config::EndpointConfig;

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub server_channel_capacity: usize,
    pub packet_queue_capacity: usize,
}

/// The configuration for the relay server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub node_id: NodeId,
    pub server_name: String,
    pub endpoint_config: EndpointConfig,
    pub server_channel_capacity: usize,
    pub packet_queue_capacity: usize,
}

impl ServerConfig {
    /// Create a new socket configuration with the default values.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            server_name: "localhost".to_string(),
            endpoint_config: EndpointConfig::new(),
            server_channel_capacity: DEFAULT_RELAY_SERVER_CHANNEL_CAPACITY,
            packet_queue_capacity: DEFAULT_RELAY_PACKET_QUEUE_CAPACITY,
        }
    }

    pub fn with_server_name(mut self, name: String) -> Self {
        self.server_name = name;
        self
    }

    pub fn with_endpoint_config(mut self, endpoint_config: EndpointConfig) -> Self {
        self.endpoint_config = endpoint_config;
        self
    }

    pub fn with_server_channel_capacity(mut self, capacity: usize) -> Self {
        self.server_channel_capacity = capacity;
        self
    }

    pub fn with_packet_queue_capacity(mut self, capacity: usize) -> Self {
        self.packet_queue_capacity = capacity;
        self
    }

    pub fn node_addr(&self) -> NodeAddr {
        NodeAddr::new(self.node_id.clone())
            .with_server_name(self.server_name.clone())
            .with_socket_addresses(self.endpoint_config.server_addresses.clone())
    }

    pub fn relay_addr(&self) -> RelayAddr {
        RelayAddr::new(self.node_id.clone())
            .with_socket_addresses(self.endpoint_config.server_addresses.clone())
    }

    pub fn relay_config(&self) -> RelayConfig {
        RelayConfig {
            server_channel_capacity: self.server_channel_capacity,
            packet_queue_capacity: self.packet_queue_capacity,
        }
    }

}
