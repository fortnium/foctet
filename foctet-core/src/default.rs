use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

// Default values for the configuration

/// The minimum size of the write(send) buffer, in bytes. (1KB)
pub const MIN_WRITE_BUFFER_SIZE: usize = 1024;
/// The default size of the write(send) buffer, in bytes. (16KB)
//pub const DEFAULT_WRITE_BUFFER_SIZE: usize = 8192;
pub const DEFAULT_WRITE_BUFFER_SIZE: usize = 16384;
/// The maximum size of the write(send) buffer, in bytes. (64KB)
pub const MAX_WRITE_BUFFER_SIZE: usize = 65536;
/// The minimum size of the read(receive) buffer, in bytes. (1KB)
pub const MIN_READ_BUFFER_SIZE: usize = 1024;
/// The default size of the read(receive) buffer, in bytes. (16KB)
//pub const DEFAULT_READ_BUFFER_SIZE: usize = 8192;
pub const DEFAULT_READ_BUFFER_SIZE: usize = 16384;
/// The maximum size of the read(receive) buffer, in bytes. (64KB)
pub const MAX_READ_BUFFER_SIZE: usize = 65536;

/// The default size of the hash buffer, in bytes. (16KB)
pub const DEFAULT_HASH_BUFFER_SIZE: usize = 16384;

/// The default connection timeout.
pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
/// The default write timeout for single write operation.
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// The default read timeout for single read operation.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// The default keep-alive interval.
pub const DEFAULT_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(20);

/// The default maximum number of concurrent streams.
pub const DEFAULT_MAX_CONCURRENT_STREAM: u32 = 256;

/// The default maximum number of retries.
pub const DEFAULT_MAX_RETRIES: usize = 3;

pub const DEFAULT_BIND_PORT: u16 = 0;
pub const LOCALHOST_BIND_V4_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
pub const DEFAULT_BIND_V4_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_BIND_PORT);
pub const LOCALHOST_BIND_V6_ADDR: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0);
pub const DEFAULT_BIND_V6_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), DEFAULT_BIND_PORT);
pub const DEFAULT_SERVER_PORT: u16 = 4432;
pub const LOCALHOST_SERVER_V4_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_SERVER_PORT);
pub const DEFAULT_SERVER_V4_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_SERVER_PORT);
pub const LOCALHOST_SERVER_V6_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), DEFAULT_SERVER_PORT);
pub const DEFAULT_SERVER_V6_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), DEFAULT_SERVER_PORT);
pub const DEFAULT_RELAY_SERVER_HOST_ADDR: &str = "relay.foctet.net";
pub const DEFAULT_RELAY_SERVER_PORT: u16 = 4433;

pub const DEFAULT_RELAY_PACKET_QUEUE_CAPACITY: usize = 1024;
pub const DEFAULT_RELAY_SERVER_CHANNEL_CAPACITY: usize = 1024;

pub const RESERVED_STREAM_ID: u32 = 0;
pub const INITIAL_STREAM_ID: u32 = 1;

/// The default blake3 hash length in bytes.
pub const DEFAULT_HASH_LEN: usize = 32;

/// The default configuration directory name.
pub const DEFAULT_CONFIG_DIR: &str = ".foctet";
