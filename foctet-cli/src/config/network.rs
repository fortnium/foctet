use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use foctet::core::{
    addr::NamedSocketAddr,
    default::{DEFAULT_RELAY_SERVER_HOST_ADDR, DEFAULT_RELAY_SERVER_PORT, DEFAULT_SERVER_PORT},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub bind_addrs: Vec<SocketAddr>,
    pub relay_addr: NamedSocketAddr,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        let any_ipv4: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let any_ipv6: IpAddr = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
        let ipv4_socket: SocketAddr = SocketAddr::new(any_ipv4, DEFAULT_SERVER_PORT);
        let ipv6_socket: SocketAddr = SocketAddr::new(any_ipv6, DEFAULT_SERVER_PORT);
        NetworkConfig {
            // The any address (both IPv4 and IPv6) is used to bind to all interfaces
            bind_addrs: vec![ipv4_socket, ipv6_socket],
            relay_addr: NamedSocketAddr::new(
                DEFAULT_RELAY_SERVER_HOST_ADDR,
                DEFAULT_RELAY_SERVER_PORT,
            ),
        }
    }
}
