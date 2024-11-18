use foctet_core::default::{DEFAULT_BIND_V4_ADDR, DEFAULT_BIND_V6_ADDR, DEFAULT_SERVER_PORT, LOCALHOST_SERVER_V4_ADDR, LOCALHOST_SERVER_V6_ADDR};
use std::{collections::BTreeSet, net::{IpAddr, SocketAddr}};

/// Get the default bind address.
/// If IPv6 is available, return the unspecified IPv6 address.
/// Otherwise, return the unspecified IPv4 address.
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

pub fn get_default_server_addrs(include_loopback: bool) -> BTreeSet<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return BTreeSet::new(),
    };
    let mut server_addrs = BTreeSet::new();
    if include_loopback {
        server_addrs.insert(LOCALHOST_SERVER_V4_ADDR);
    }
    for ipv4net in default_interface.ipv4.iter() {
        server_addrs.insert(SocketAddr::new(IpAddr::V4(ipv4net.addr()), DEFAULT_SERVER_PORT));
    }
    if include_loopback {
        server_addrs.insert(LOCALHOST_SERVER_V6_ADDR);
    }
    for ipv6net in default_interface.ipv6.iter() {
        server_addrs.insert(SocketAddr::new(IpAddr::V6(ipv6net.addr()), DEFAULT_SERVER_PORT));
    }
    server_addrs
}

pub fn get_default_ipv4_server_addrs(include_loopback: bool) -> BTreeSet<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return BTreeSet::new(),
    };
    let mut server_addrs = BTreeSet::new();
    if include_loopback {
        server_addrs.insert(LOCALHOST_SERVER_V4_ADDR);
    }
    for ipv4net in default_interface.ipv4.iter() {
        server_addrs.insert(SocketAddr::new(IpAddr::V4(ipv4net.addr()), DEFAULT_SERVER_PORT));
    }
    server_addrs
}

pub fn get_default_ipv6_server_addrs(include_loopback: bool) -> BTreeSet<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return BTreeSet::new(),
    };
    let mut server_addrs = BTreeSet::new();
    if include_loopback {
        server_addrs.insert(LOCALHOST_SERVER_V6_ADDR);
    }
    for ipv6net in default_interface.ipv6.iter() {
        server_addrs.insert(SocketAddr::new(IpAddr::V6(ipv6net.addr()), DEFAULT_SERVER_PORT));
    }
    server_addrs
}
