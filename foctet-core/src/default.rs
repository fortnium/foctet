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
pub const DEFAULT_RELAY_SERVER_HOST_ADDR: &str = "relay.foctet.net";
pub const DEFAULT_RELAY_SERVER_PORT: u16 = 4433;

// The foctet user configuration directory structure
/*
$HOME/.foctet/
├── foctet-config.toml              # Main configuration file
├── keys/                    # Keys directory
│   ├── id_ed25519.p8        # Default keypair file
├── tls/                        # TLS certificate and key files
│   ├── cert.chain.pem                # Self-signed or user-provided TLS certificate
│   └── key.pem                 # Private key corresponding to the TLS certificate
├── logs/                    # Log files directory
│   ├── foctet-YYYY-MM-DD.log           # Log file that can be rotated
├── temp/                    # Temporary directory
│   ├── uploads/             # Uploads directory
│   └── downloads/           # Donwloads directory
├── db/                      # Database directory
│   └── foctet.db            # SQLite database file
└── cache/                   # Cache directory
    └── metadata_cache/      # Metadata cache directory
        ├── file1.json       # Metadata cache file
        └── file2.json       # Metadata cache file
*/

/// The default configuration directory name.
pub const DEFAULT_CONFIG_DIR: &str = ".foctet";
pub const DEFAULT_KEYS_DIR: &str = "keys";
pub const DEFAULT_TEMP_DIR: &str = "temp";
pub const DEFAULT_LOG_DIR: &str = "logs";
pub const DEFAULT_LOG_FILE: &str = "foctet.log";
pub const DEFAULT_LOG_LEVEL: &str = "info";
pub const DEFAULT_CONFIG_FILE: &str = "foctet-config.toml";
pub const DEFAULT_DATABASE_DIR: &str = "db";
pub const DEFAULT_DATABASE_FILE: &str = "foctet.db";
pub const DEFAULT_CACHE_DIR: &str = "cache";
pub const DEFAULT_METADATA_CACHE_DIR: &str = "metadata_cache";
/// The default cache expiration time in seconds. (1 hour)
pub const DEFAULT_CACHE_EXPIRATION: u64 = 3600;
/// The default maximum cache size in bytes. (50MB)
pub const DEFAULT_CACHE_MAX_SIZE: u64 = 50 * 1024 * 1024;
pub const DEFAULT_OUTPUT_DIR: &str = "output";
pub const DEFAULT_TLS_DIR: &str = "tls";
pub const DEFAULT_CERT_FILE: &str = "cert.chain.pem";
pub const DEFAULT_KEY_FILE: &str = "key.pem";

/// The default keypair (PKCS#8 document) file name. 
/// The PKCS#8 document is a v2 `OneAsymmetricKey` with the public key,
/// as described in [RFC 5958 Section 2]; see [RFC 8410 Section 10.3]
///
/// [RFC 5958 Section 2]: https://tools.ietf.org/html/rfc5958#section-2
/// [RFC 8410 Section 10.3]: https://tools.ietf.org/html/rfc8410#section-10.3
pub const DEFAULT_KEYPAIR_FILE: &str = "id_ed25519.p8";

/// The default blake3 hash length in bytes.
pub const DEFAULT_HASH_LEN: usize = 32;
