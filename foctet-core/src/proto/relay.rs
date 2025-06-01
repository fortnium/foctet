use std::fmt;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use crate::{addr::node::NodeAddr, id::{NodeId, UuidV4}};
use anyhow::Result;

#[derive(Deserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TunnelId(pub UuidV4);

impl TunnelId {
    pub fn new() -> Self {
        Self(UuidV4::new())
    }
    pub fn id(&self) -> UuidV4 {
        self.0
    }
}

impl fmt::Display for TunnelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_hex())
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
pub struct TunnelInfo {
    pub src_node: NodeId,
    pub dst_node: NodeId,
    pub tunnel_id: TunnelId,
}

impl TunnelInfo {
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

impl fmt::Display for TunnelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TunnelRequest(src_node: {}, dst_node: {})", self.src_node, self.dst_node)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelRequest {
    Create(TunnelInfo),
    Close(TunnelId),
}

impl TunnelRequest {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelResponse {
    Created(TunnelId),
    Closed(TunnelId),
    Error(String),
}

impl TunnelResponse {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (msg, _len) = bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;
        Ok(msg)
    }

    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(bincode::serde::encode_to_vec(self, bincode::config::standard())?))
    }
}
