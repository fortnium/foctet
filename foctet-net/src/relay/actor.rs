use std::path::PathBuf;

use foctet_core::{content::ContentId, frame::Frame, node::NodeAddr};

#[derive(Debug)]
pub enum RelayActorMessage {
    // Relay-specific control
    Register(NodeAddr),
    Remove(NodeAddr),
    RelayConnect(NodeAddr),
    RelayConnected(NodeAddr),
    RelayDisconnect(NodeAddr),
    RelayDisconnected(NodeAddr),
    // Relay data transfer
    RelayData(ContentId, Vec<u8>),
    SendFrame(ContentId, Frame),
    RelayFile(ContentId, PathBuf),
}
