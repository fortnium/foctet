use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Returns [`true`] if the address appears to be globally routable.
pub fn is_global_ip(ip_addr: &IpAddr) -> bool {
    match ip_addr {
        IpAddr::V4(ip) => is_global_ipv4(ip),
        IpAddr::V6(ip) => is_global_ipv6(ip),
    }
}

/// Returns [`true`] if the address appears to be globally reachable
/// as specified by the [IANA IPv4 Special-Purpose Address Registry].
pub fn is_global_ipv4(ipv4_addr: &Ipv4Addr) -> bool {
    !(ipv4_addr.octets()[0] == 0 // "This network"
        || ipv4_addr.is_private()
        || is_shared_ipv4(ipv4_addr)
        || ipv4_addr.is_loopback()
        || ipv4_addr.is_link_local()
        // addresses reserved for future protocols (`192.0.0.0/24`)
        // .9 and .10 are documented as globally reachable so they're excluded
        || (
            ipv4_addr.octets()[0] == 192 && ipv4_addr.octets()[1] == 0 && ipv4_addr.octets()[2] == 0
            && ipv4_addr.octets()[3] != 9 && ipv4_addr.octets()[3] != 10
        )
        || ipv4_addr.is_documentation()
        || is_benchmarking_ipv4(ipv4_addr)
        || is_reserved_ipv4(ipv4_addr)
        || ipv4_addr.is_broadcast())
}

/// Returns [`true`] if the address appears to be globally reachable
/// as specified by the [IANA IPv6 Special-Purpose Address Registry].
pub fn is_global_ipv6(ipv6_addr: &Ipv6Addr) -> bool {
    !(ipv6_addr.is_unspecified()
        || ipv6_addr.is_loopback()
        // IPv4-mapped Address (`::ffff:0:0/96`)
        || matches!(ipv6_addr.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
        // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
        || matches!(ipv6_addr.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
        // Discard-Only Address Block (`100::/64`)
        || matches!(ipv6_addr.segments(), [0x100, 0, 0, 0, _, _, _, _])
        // IETF Protocol Assignments (`2001::/23`)
        || (matches!(ipv6_addr.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
            && !(
                // Port Control Protocol Anycast (`2001:1::1`)
                u128::from_be_bytes(ipv6_addr.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                || u128::from_be_bytes(ipv6_addr.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                // AMT (`2001:3::/32`)
                || matches!(ipv6_addr.segments(), [0x2001, 3, _, _, _, _, _, _])
                // AS112-v6 (`2001:4:112::/48`)
                || matches!(ipv6_addr.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                // ORCHIDv2 (`2001:20::/28`)
                // Drone Remote ID Protocol Entity Tags (DETs) Prefix (`2001:30::/28`)`
                || matches!(ipv6_addr.segments(), [0x2001, b, _, _, _, _, _, _] if b >= 0x20 && b <= 0x3F)
            ))
        // 6to4 (`2002::/16`) – it's not explicitly documented as globally reachable,
        // IANA says N/A.
        || matches!(ipv6_addr.segments(), [0x2002, _, _, _, _, _, _, _])
        || is_documentation_ipv6(ipv6_addr)
        || ipv6_addr.is_unique_local()
        || ipv6_addr.is_unicast_link_local())
}

/// Returns [`true`] if this address is part of the Shared Address Space defined in
/// [IETF RFC 6598] (`100.64.0.0/10`).
///
/// [IETF RFC 6598]: https://tools.ietf.org/html/rfc6598
fn is_shared_ipv4(ipv4_addr: &Ipv4Addr) -> bool {
    ipv4_addr.octets()[0] == 100 && (ipv4_addr.octets()[1] & 0b1100_0000 == 0b0100_0000)
}

/// Returns [`true`] if this address part of the `198.18.0.0/15` range, which is reserved for
/// network devices benchmarking.
fn is_benchmarking_ipv4(ipv4_addr: &Ipv4Addr) -> bool {
    ipv4_addr.octets()[0] == 198 && (ipv4_addr.octets()[1] & 0xfe) == 18
}

/// Returns [`true`] if this address is reserved by IANA for future use.
fn is_reserved_ipv4(ipv4_addr: &Ipv4Addr) -> bool {
    ipv4_addr.octets()[0] & 240 == 240 && !ipv4_addr.is_broadcast()
}

/// Returns [`true`] if this is an address reserved for documentation
/// (`2001:db8::/32` and `3fff::/20`).
fn is_documentation_ipv6(ipv6_addr: &Ipv6Addr) -> bool {
    matches!(ipv6_addr.segments(), [0x2001, 0xdb8, ..] | [0x3fff, 0..=0x0fff, ..])
}
