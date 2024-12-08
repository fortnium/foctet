use std::path::PathBuf;

use foctet_core::{frame::{Frame, OperationId}, node::{NodeAddr, NodeId}};

use crate::connection::ConnectionInfo;

/// Actor messages
#[derive(Debug)]
pub enum ActorMessage {
    /// Session management related commands
    SessionManagement(SessionCommand),
    /// Data transfer related commands
    DataTransfer {
        operation_id: OperationId,
        target_node: NodeId,
        payload: TransferPayload,
    },
    /// The received frame from the target node
    ReceivedFrame {
        src_node_id: NodeId,
        frame: Frame,
    },
    /// Control related commands
    Control(ControlCommand),
    /// Confirmation and response messages
    Ack(AckMessage),
}

/// Relay Actor messages
#[derive(Debug)]
pub enum RelayActorMessage {
    /// Session management related commands
    SessionManagement(SessionCommand),
    /// Data transfer related commands
    DataTransfer {
        operation_id: OperationId,
        target_node: NodeId,
        payload: TransferPayload,
    },
    /// Data reception related commands
    DataReceive {
        source_node: NodeId,
        receive_type: ReceiveType,
    },
    /// Control related commands
    Control(ControlCommand),
    /// Confirmation and response messages
    Ack(AckMessage),
}

/// Session management related commands
#[derive(Debug)]
pub enum SessionCommand {
    Connect(NodeAddr),
    Disconnect(NodeId),
    Register(NodeAddr),
    Remove(NodeId),
}

/// Data transfer related commands
#[derive(Debug)]
pub enum TransferPayload {
    Frame(Frame),
    File(PathBuf), // Path to the file
}

/// Data reception related commands
#[derive(Debug)]
pub enum ReceiveType {
    Frame,
    File(PathBuf), // Path to save the file
}

/// Control related commands
#[derive(Debug)]
pub enum ControlCommand {
    Shutdown,
}

/// Confirmation and response messages
#[derive(Debug)]
pub enum AckMessage {
    Success,
    Error(anyhow::Error),
    Connected(ConnectionInfo),
    Disconnected(NodeId),
    TransferComplete {
        operation_id: OperationId,
        node_id: NodeId,
    },
    TransferError {
        operation_id: OperationId,
        node_id: NodeId,
        error: anyhow::Error,
    },
    Frame {
        node_id: NodeId,
        frame: Frame,
    },
    File {
        node_id: NodeId,
        path: PathBuf,
        byte_size: u64,
    },
    ShutdownComplete,
}
