use foctet_core::{default::{DEFAULT_BIND_V4_ADDR, DEFAULT_BIND_V6_ADDR, DEFAULT_SERVER_V4_ADDR, DEFAULT_SERVER_V6_ADDR}, ip};
use netdev::Interface;
use stackaddr::StackAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use ipnet::{Ipv4Net, Ipv6Net};

/// Get the default unspecified bind address.
/// If IPv6 is available, return the unspecified IPv6 address.
/// Otherwise, return the unspecified IPv4 address.
#[allow(dead_code)]
pub(crate) fn get_unspecified_bind_addr() -> SocketAddr {
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

/// Get the default unspecified server address.
/// If IPv6 is available, return the unspecified IPv6 address.
/// Otherwise, return the unspecified IPv4 address.
pub(crate) fn get_unspecified_server_addr() -> SocketAddr {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return DEFAULT_SERVER_V4_ADDR,
    };
    if default_interface.ipv6.len() > 0 {
        return DEFAULT_SERVER_V6_ADDR;
    } else {
        return DEFAULT_SERVER_V4_ADDR;
    }
}

pub(crate) fn get_default_server_addrs(port: u16, include_loopback: bool) -> Vec<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return Vec::new(),
    };
    let mut server_addrs = Vec::new();
    if include_loopback {
        server_addrs.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    for ipv4net in default_interface.ipv4.iter() {
        server_addrs.push(SocketAddr::new(IpAddr::V4(ipv4net.addr()), port));
    }
    if include_loopback {
        server_addrs.push(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port));
    }
    for ipv6net in default_interface.ipv6.iter() {
        server_addrs.push(SocketAddr::new(IpAddr::V6(ipv6net.addr()), port));
    }
    server_addrs
}

#[allow(dead_code)]
pub(crate) fn get_default_ipv4_server_addrs(port: u16, include_loopback: bool) -> Vec<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return Vec::new(),
    };
    let mut server_addrs = Vec::new();
    if include_loopback {
        server_addrs.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    for ipv4net in default_interface.ipv4.iter() {
        server_addrs.push(SocketAddr::new(IpAddr::V4(ipv4net.addr()), port));
    }
    server_addrs
}

#[allow(dead_code)]
pub(crate) fn get_default_ipv6_server_addrs(port: u16, include_loopback: bool) -> Vec<SocketAddr> {
    let default_interface = match netdev::get_default_interface() {
        Ok(interface) => interface,
        Err(_) => return Vec::new(),
    };
    let mut server_addrs = Vec::new();
    if include_loopback {
        server_addrs.push(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port));
    }
    for ipv6net in default_interface.ipv6.iter() {
        server_addrs.push(SocketAddr::new(IpAddr::V6(ipv6net.addr()), port));
    }
    server_addrs
}

/// Check if the target IP address is in the same network as the interface.
pub(crate) fn in_same_net(target_ip: &IpAddr, iface: &Interface) -> bool {
    match target_ip {
        IpAddr::V4(target_v4) => {
            for ipv4net in &iface.ipv4 {
                let net = match Ipv4Net::new(ipv4net.addr(), ipv4net.prefix_len()) {
                    Ok(net) => net,
                    Err(_) => continue,
                };
                if net.contains(target_v4) {
                    return true;
                }
            }
        }
        IpAddr::V6(target_v6) => {
            for ipv6net in &iface.ipv6 {
                let net = match Ipv6Net::new(ipv6net.addr(), ipv6net.prefix_len()) {
                    Ok(net) => net,
                    Err(_) => continue,
                };
                if net.contains(target_v6) {
                    return true;
                }
            }
        }
    }
    false
}

/// Rank the IP address based on its type and reachability.
/// The ranking is as follows:
/// 0 - Not a global IP
/// 1 - Global IP
/// 2 - In the same network as the interface
/// 3 - Loopback IP
fn rank_ip(ip: &IpAddr, iface: &Interface) -> u8 {
    if ip.is_loopback() {
        3
    } else if in_same_net(ip, iface) {
        2
    } else if ip::is_global_ip(ip) {
        1
    } else {
        0
    }
}

/// Pick the best address from a list of addresses.
/// The best address is the one with the highest rank.
/// If there are multiple addresses with the same rank,
/// the first one in the list is returned.
#[allow(dead_code)]
pub(crate) fn pick_best_addr(addrs: &[StackAddr], iface: &Interface) -> Option<StackAddr> {
    addrs.iter()
        .filter_map(|addr| addr.ip().map(|ip| (addr, rank_ip(&ip, iface))))
        .max_by_key(|&(_, rank)| rank)
        .map(|(addr, _)| addr.clone())
}

/// Sort the addresses by their reachability.
/// The addresses are sorted in descending order of their rank.
/// The ranking is as follows:
/// 0 - Not a global IP
/// 1 - Global IP
/// 2 - In the same network as the interface
/// 3 - Loopback IP
/// The addresses with the same rank are sorted by their IP address.
/// The addresses with the highest rank are at the beginning of the list.
pub(crate) fn sort_addrs_by_reachability(
    addrs: &[StackAddr],
    iface: &Interface,
) -> Vec<StackAddr> {
    let mut ranked: Vec<_> = addrs
        .iter()
        .filter_map(|addr| addr.ip().map(|ip| (addr, rank_ip(&ip, iface))))
        .collect();

    ranked.sort_by_key(|&(_, rank)| std::cmp::Reverse(rank));

    ranked.into_iter().map(|(addr, _)| addr).cloned().collect()
}
