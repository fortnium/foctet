use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::collections::BTreeSet;

/// Sorts a list of SocketAddr based on priority:
/// 1. Localhost
/// 2. Link-local IPv6
/// 3. Private networks
/// 4. Global IPv6
/// 5. Public IPv4
/// 6. Other addresses
pub fn sort_socket_addrs(addrs: &BTreeSet<SocketAddr>) -> Vec<SocketAddr> {
    let mut addrs: Vec<_> = addrs.iter().cloned().collect();

    addrs.sort_by(|a, b| {
        let a_priority = get_addr_priority(a.ip());
        let b_priority = get_addr_priority(b.ip());

        // Compare by priority first
        a_priority.cmp(&b_priority)
            // If priority is the same, prefer IPv6 over IPv4
            .then_with(|| match (a.ip(), b.ip()) {
                (IpAddr::V6(_), IpAddr::V4(_)) => std::cmp::Ordering::Less,
                (IpAddr::V4(_), IpAddr::V6(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
            // Lastly, compare by port (for determinism)
            .then_with(|| a.port().cmp(&b.port()))
    });

    addrs
}

/// Returns a priority value for a given IP address.
/// Lower values indicate higher priority.
fn get_addr_priority(ip: IpAddr) -> u8 {
    match ip {
        IpAddr::V4(addr) => {
            if addr == Ipv4Addr::LOCALHOST {
                0 // Localhost IPv4
            } else if addr.is_private() {
                match addr.octets() {
                    [192, 168, ..] => 2,         // Private IPv4: 192.168.x.x
                    [172, 16..=31, ..] => 3,     // Private IPv4: 172.16.x.x - 172.31.x.x
                    [10, ..] => 4,               // Private IPv4: 10.x.x.x
                    _ => 5,                      // Other private IPv4
                }
            } else {
                6 // Public IPv4
            }
        }
        IpAddr::V6(addr) => {
            if addr == Ipv6Addr::LOCALHOST {
                0 // Localhost IPv6
            } else if foctet_core::ip::is_link_local_ipv6(&addr) {
                1 // Link-local IPv6
            } else if foctet_core::ip::is_unique_local_ipv6(&addr) {
                3 // Unique local address (ULA)
            } else if foctet_core::ip::is_global_ipv6(&addr) {
                5 // Global IPv6
            } else if addr.is_multicast() {
                7 // Multicast address
            } else {
                8 // Other IPv6
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_socket_addrs() {
        use std::net::{Ipv4Addr, Ipv6Addr};
    
        let mut addrs = BTreeSet::new();
        addrs.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443)); // Localhost IPv4
        addrs.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 443)); // Private IPv4
        addrs.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)), 443)); // Private IPv4
        addrs.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 443)); // Private IPv4
        addrs.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443)); // Public IPv4
        addrs.insert(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 443)); // Localhost IPv6
        addrs.insert(SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)), 443)); // Unique Local Address (ULA)
        addrs.insert(SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)), 443)); // Global IPv6
        addrs.insert(SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)), 443)); // Link-local IPv6
    
        let sorted = sort_socket_addrs(&addrs);
    
        let expected = vec![
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 443),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)), 443), // Link-local IPv6
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 443), // Private IPv4
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)), 443), // Unique Local Address (ULA)
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)), 443), // Private IPv4
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 443), // Private IPv4
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)), 443), // Global IPv6
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443), // Public IPv4
        ];
    
        assert_eq!(sorted, expected);
    }
}
