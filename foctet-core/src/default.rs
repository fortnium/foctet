use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

// Default values for the configuration

/// The minimum size of the write(send) buffer, in bytes. (1KB)
pub const MIN_WRITE_BUFFER_SIZE: usize = 1024;
/// The default size of the write(send) buffer, in bytes. (8KB)
pub const DEFAULT_WRITE_BUFFER_SIZE: usize = 8192;
/// The maximum size of the write(send) buffer, in bytes. (64KB)
pub const MAX_WRITE_BUFFER_SIZE: usize = 65536;
/// The minimum size of the read(receive) buffer, in bytes. (1KB)
pub const MIN_READ_BUFFER_SIZE: usize = 1024;
/// The default size of the read(receive) buffer, in bytes. (8KB)
pub const DEFAULT_READ_BUFFER_SIZE: usize = 8192;
/// The maximum size of the read(receive) buffer, in bytes. (64KB)
pub const MAX_READ_BUFFER_SIZE: usize = 65536;

/// The default size of the hash buffer, in bytes. (16KB)
pub const DEFAULT_HASH_BUFFER_SIZE: usize = 16384;

/// The default connection timeout.
pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
/// The default send(write) timeout for single write operation.
pub const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// The default receive(read) timeout for single read operation.
pub const DEFAULT_RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);
/// The default keep-alive interval.
pub const DEFAULT_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(20);

/// The default maximum number of retries.
pub const DEFAULT_MAX_RETRIES: usize = 3;

pub const DEFAULT_BIND_PORT: u16 = 0;
pub const DEFAULT_BIND_V4_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_BIND_PORT);
pub const DEFAULT_BIND_V6_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), DEFAULT_BIND_PORT);
pub const DEFAULT_SERVER_PORT: u16 = 4432;
pub const DEFAULT_SERVER_V4_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_SERVER_PORT);
pub const DEFAULT_SERVER_V6_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), DEFAULT_SERVER_PORT);
pub const DEFAULT_RELAY_SERVER_PORT: u16 = 4433;

pub const DEFAULT_CONFIG_DIR: &str = ".foctet";

/// The default keypair (PKCS#8 document) file name. 
/// The PKCS#8 document is a v2 `OneAsymmetricKey` with the public key,
/// as described in [RFC 5958 Section 2]; see [RFC 8410 Section 10.3]
///
/// [RFC 5958 Section 2]: https://tools.ietf.org/html/rfc5958#section-2
/// [RFC 8410 Section 10.3]: https://tools.ietf.org/html/rfc8410#section-10.3
pub const DEFAULT_KEYPAIR_FILE: &str = "id_ed25519.p8";
