use std::net::{IpAddr, SocketAddr};

use netdev::Interface;

/// Filters a list of socket addresses to only include those that are reachable IP addresses.
/// This function uses the system's network interfaces to determine reachability.
pub fn filter_reachable_addrs(addrs: Vec<SocketAddr>, include_loopback: bool) -> Vec<SocketAddr> {
    let interfaces = netdev::get_interfaces();
    let ipv4_networks = collect_ipv4_networks(&interfaces);
    let ipv6_networks = collect_ipv6_networks(&interfaces);
    let mut reachable_addrs = Vec::new();
    for addr in addrs {
        match addr.ip() {
            IpAddr::V4(ipv4) => {
                if !include_loopback && ipv4.is_loopback() {
                    continue;
                }
                if is_ipv4_reachable(&ipv4, &ipv4_networks) {
                    reachable_addrs.push(addr);
                }
            }
            IpAddr::V6(ipv6) => {
                if !include_loopback && ipv6.is_loopback() {
                    continue;
                }
                if is_ipv6_reachable(&ipv6, &ipv6_networks) {
                    reachable_addrs.push(addr);
                }
            }
        }
    }
    reachable_addrs
}

fn collect_ipv4_networks(interfaces: &[Interface]) -> Vec<netdev::ipnet::Ipv4Net> {
    interfaces
        .iter()
        .flat_map(|interface| interface.ipv4.iter().cloned())
        .collect()
}

fn collect_ipv6_networks(interfaces: &[Interface]) -> Vec<netdev::ipnet::Ipv6Net> {
    interfaces
        .iter()
        .flat_map(|interface| interface.ipv6.iter().cloned())
        .collect()
}

/// Checks if an IPv4 address is reachable based on the system's interfaces.
fn is_ipv4_reachable(ipv4: &std::net::Ipv4Addr, networks: &[netdev::ipnet::Ipv4Net]) -> bool {
    if foctet_core::ip::is_global_ipv4(ipv4) {
        return true;
    }
    for network in networks {
        if network.contains(ipv4) {
            return true;
        }
    }
    false
}

/// Checks if an IPv6 address is reachable based on the system's interfaces.
fn is_ipv6_reachable(ipv6: &std::net::Ipv6Addr, interfaces: &[netdev::ipnet::Ipv6Net]) -> bool {
    if foctet_core::ip::is_global_ipv6(ipv6) {
        return true;
    }
    for network in interfaces {
        if network.contains(ipv6) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_reachable_addrs() {
        let addrs = vec![
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 10, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 11, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(172, 16, 0, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1)), 443),
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(1, 1, 1, 1)), 443),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 443),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)), 443),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)), 443),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)), 443),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0x4700, 0, 0, 0, 1111)), 443),
        ];

        let reachable_addrs = filter_reachable_addrs(addrs, true);

        println!("{:?}", reachable_addrs);

    }
}
