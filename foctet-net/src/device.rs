use foctet_core::default::{DEFAULT_BIND_V4_ADDR, DEFAULT_BIND_V6_ADDR};
use std::net::SocketAddr;

pub fn get_default_bind_addr() -> SocketAddr {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return DEFAULT_BIND_V4_ADDR,
    };
    if default_interface.ipv6.len() > 0 {
        return DEFAULT_BIND_V6_ADDR;
    } else {
        return DEFAULT_BIND_V4_ADDR;
    }
}
