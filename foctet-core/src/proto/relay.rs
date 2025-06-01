use std::fmt;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use crate::{addr::node::NodeAddr, id::NodeId};
use anyhow::Result;

#[derive(Deserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TunnelId(pub u32);

impl TunnelId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    pub fn next(&self) -> Self {
        TunnelId(self.0.checked_add(1).unwrap_or(0))
    }
    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&mut self, n: u32) -> Self {
        let old = self.0;
        self.0 = self.0.checked_add(n).unwrap_or(0);
        TunnelId(old)
    }
}

impl fmt::Display for TunnelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterRequest {
    ControlStream(NodeAddr),
    DataStream(TunnelId),
}

impl RegisterRequest {    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterResponse {
    ControlStreamAccepted,
    DataStreamAccepted(TunnelId),
    Rejected(String),
}

impl RegisterResponse {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRequest {
    pub src_node: NodeId,
    pub dst_node: NodeId,
    pub tunnel_id: TunnelId,
}

impl TunnelRequest {
    pub fn new(src_node: NodeId, dst_node: NodeId, tunnel_id: TunnelId) -> Self {
        Self {
            src_node,
            dst_node,
            tunnel_id,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}

impl fmt::Display for TunnelRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TunnelRequest(src_node: {}, dst_node: {})", self.src_node, self.dst_node)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelResponse {
    pub success: bool,
    pub tunnel_id: TunnelId,
    pub reason: Option<String>,
}

impl TunnelResponse {
    pub fn new(success: bool, tunnel_id: TunnelId, reason: Option<String>) -> Self {
        Self { success, tunnel_id, reason }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}

impl fmt::Display for TunnelResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TunnelResponse(success: {}, tunnel_id: {}, reason: {:?})", self.success, self.tunnel_id, self.reason)
    }
}
