use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::Result;
use foctet_core::id::NodeId;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::error::ResolveError;
use hickory_resolver::system_conf;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::{
    lookup::{Ipv4Lookup, Ipv6Lookup, TxtLookup},
    lookup_ip::LookupIp,
};
use stackaddr::{Protocol, StackAddr};

/// The prefix for `NodeId` TXT record lookups.
const DNS_NODEID_PREFIX: &str = "_nodeid.";

/// The result of a successful resolution of a `Resolver`.
pub enum Resolved {
    /// The given name has been resolved to a single `IpAddr`.
    One(IpAddr),
    /// The given name has been resolved to multiple `IpAddr`s.
    Many(Vec<IpAddr>),
    /// The given name has been resolved to a new list of `NodeId`s
    /// obtained from DNS TXT records representing possible alternatives.
    NodeIds(Vec<NodeId>),
}

#[derive(Debug, Clone)]
pub enum ResolverOption {
    Default,
    System,
    Cloudflare,
    Google,
    Quad9,
    Custom(ResolverConfig, ResolverOpts),
}

pub struct Resolver {
    resolver: TokioAsyncResolver,
}

impl Resolver {
    /// Creates a new [`Resolver`] with the default resolver configuration (Cloudflare) and options.
    pub fn new() -> Result<Self> {
        Ok(Self {
            resolver: TokioAsyncResolver::tokio(
                ResolverConfig::cloudflare(),
                ResolverOpts::default(),
            ),
        })
    }
    /// Creates a [`Resolver`] with a custom resolver configuration and options.
    pub fn custom(cfg: ResolverConfig, opts: ResolverOpts) -> Self {
        Self {
            resolver: TokioAsyncResolver::tokio(cfg, opts),
        }
    }
    /// Creates a new [`Resolver`] from the OS's DNS configuration and defaults.
    pub fn system() -> Result<Self> {
        let (cfg, opts) = system_conf::read_system_conf()?;
        Ok(Self::custom(cfg, opts))
    }

    pub fn from_option(option: ResolverOption) -> Result<Self> {
        match option {
            ResolverOption::Default => Self::new(),
            ResolverOption::System => Self::system(),
            ResolverOption::Cloudflare => Ok(Self::custom(
                ResolverConfig::cloudflare(),
                ResolverOpts::default(),
            )),
            ResolverOption::Google => Ok(Self::custom(
                ResolverConfig::google(),
                ResolverOpts::default(),
            )),
            ResolverOption::Quad9 => Ok(Self::custom(
                ResolverConfig::quad9(),
                ResolverOpts::default(),
            )),
            ResolverOption::Custom(cfg, opts) => Ok(Self::custom(cfg, opts)),
        }
    }

    pub async fn lookup_ip(&self, name: String) -> Result<Vec<IpAddr>, ResolveError> {
        let lookup: LookupIp = self.resolver.lookup_ip(name).await?;
        let mut ips = Vec::new();
        for ip in lookup.into_iter() {
            ips.push(ip);
        }
        Ok(ips)
    }

    pub async fn ipv4_lookup(&self, name: String) -> Result<Vec<Ipv4Addr>, ResolveError> {
        let lookup: Ipv4Lookup = self.resolver.ipv4_lookup(name).await?;
        let mut ips = Vec::new();
        for ip in lookup.into_iter() {
            ips.push(ip.0);
        }
        Ok(ips)
    }

    pub async fn ipv6_lookup(&self, name: String) -> Result<Vec<Ipv6Addr>, ResolveError> {
        let lookup: Ipv6Lookup = self.resolver.ipv6_lookup(name).await?;
        let mut ips = Vec::new();
        for ip in lookup.into_iter() {
            ips.push(ip.0);
        }
        Ok(ips)
    }

    pub async fn txt_lookup(&self, name: String) -> Result<TxtLookup, ResolveError> {
        self.resolver.txt_lookup(name).await
    }

    pub async fn resolve(&self, name: String) -> Result<Resolved, ResolveError> {
        let ips = self.lookup_ip(name.clone()).await?;
        if ips.len() == 1 {
            return Ok(Resolved::One(ips[0]));
        }
        if ips.len() > 1 {
            return Ok(Resolved::Many(ips));
        }
        let name = [DNS_NODEID_PREFIX, &name].concat();
        let txts = self.txt_lookup(name).await?;
        let mut node_ids = Vec::new();
        for txt in txts {
            // Parse the TXT record as a `NodeId`. from base32.
            if let Some(chars) = txt.txt_data().first() {
                let s = match std::str::from_utf8(chars) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                match NodeId::from_base32(s) {
                    Ok(node_id) => {
                        node_ids.push(node_id);
                    }
                    Err(_) => {
                        // Ignore invalid `NodeId`s.
                    }
                }
            }
        }
        Ok(Resolved::NodeIds(node_ids))
    }

    pub async fn resolve_addr(&self, addr: &mut StackAddr) -> Result<()> {
        if addr.resolved() {
            return Ok(());
        }
        let dns = if let Some(dns) = addr.get_dns() {
            dns
        } else {
            return Err(anyhow::anyhow!("Invalid address(No DNS)"));
        };
        match dns {
            Protocol::Dns(name) => {
                let ips = self.lookup_ip(name.to_string()).await?;
                for ip in ips {
                    addr.resolve(ip);
                    break;
                }
            }
            Protocol::Dns4(name) => {
                let ips = self.ipv4_lookup(name.to_string()).await?;
                for ip in ips {
                    addr.resolve(IpAddr::V4(ip));
                    break;
                }
            }
            Protocol::Dns6(name) => {
                let ips = self.ipv6_lookup(name.to_string()).await?;
                for ip in ips {
                    addr.resolve(IpAddr::V6(ip));
                    break;
                }
            }
            _ => {
                return Err(anyhow::anyhow!("Invalid address. Not supported"));
            }
        }
        if addr.resolved() {
            return Ok(());
        } else {
            return Err(anyhow::anyhow!("Invalid address. Not resolved"));
        }
    }
}
