use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamedSocketAddr {
    pub host: String,
    pub port: u16,
}

impl NamedSocketAddr {
    /// Creates a new `NamedSocketAddr` with the specified host and port.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    /// Resolves the `NamedSocketAddr` to a `SocketAddr`, if the host can be resolved to an IP address.
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        let address = format!("{}:{}", self.host, self.port);
        address.to_socket_addrs().ok()?.next()
    }

    pub fn to_socket_addrs(&self) -> Option<impl Iterator<Item = SocketAddr>> {
        let address = format!("{}:{}", self.host, self.port);
        address.to_socket_addrs().ok()
    }
}

impl fmt::Display for NamedSocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

// Implement Serialize manually to output as "host:port" format
impl Serialize for NamedSocketAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let address = format!("{}:{}", self.host, self.port);
        serializer.serialize_str(&address)
    }
}

// Implement Deserialize manually to parse "host:port" format
impl<'de> Deserialize<'de> for NamedSocketAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let mut parts = s.split(':');

        let host = parts
            .next()
            .ok_or_else(|| serde::de::Error::custom("Missing host"))?
            .to_string();
        let port = parts
            .next()
            .ok_or_else(|| serde::de::Error::custom("Missing port"))?
            .parse::<u16>()
            .map_err(|_| serde::de::Error::custom("Invalid port"))?;

        Ok(NamedSocketAddr { host, port })
    }
}

// Implement FromStr for clap compatibility
impl FromStr for NamedSocketAddr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let host = parts.next().ok_or_else(|| "Missing host".to_string())?;
        let port = parts
            .next()
            .ok_or_else(|| "Missing port".to_string())?
            .parse::<u16>()
            .map_err(|_| "Invalid port".to_string())?;

        if parts.next().is_some() {
            return Err("Too many parts, expected 'host:port' format".to_string());
        }

        Ok(NamedSocketAddr::new(host, port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
    struct TestConfig {
        named_socket_addr: NamedSocketAddr,
    }

    #[test]
    fn test_named_socket_addr_json() {
        let addr = NamedSocketAddr::new("relay.foctet.net", 4433);
        let serialized = serde_json::to_string(&addr).unwrap();
        assert_eq!(serialized, "\"relay.foctet.net:4433\"");
        let deserialized: NamedSocketAddr = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, addr);
    }

    #[test]
    fn test_named_socket_addr_toml() {
        let addr = NamedSocketAddr::new("relay.foctet.net", 4433);
        let test_config = TestConfig {
            named_socket_addr: addr,
        };
        let serialized = toml::to_string(&test_config).unwrap();
        assert_eq!(
            serialized,
            "named_socket_addr = \"relay.foctet.net:4433\"\n"
        );
        let deserialized: TestConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized, test_config);
    }

    #[test]
    fn test_named_socket_addr_bin() {
        let addr = NamedSocketAddr::new("relay.foctet.net", 4433);
        let serialized = bincode::serialize(&addr).unwrap();
        let deserialized: NamedSocketAddr = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized, addr);
    }
}
