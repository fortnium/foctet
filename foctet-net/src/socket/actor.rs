use std::path::PathBuf;

use foctet_core::{content::ContentId, frame::Frame, node::NodeAddr};

#[derive(Debug)]
pub enum ActorMessage {
    // Node actor control
    Spawn,
    Spawned,
    // Connection and stream control
    Connect(NodeAddr),
    Connected(NodeAddr),
    Listen,
    Listening,
    Accepted(NodeAddr),
    Disconnect,
    Disconnected,
    OpenStream,
    StreamOpened,
    CloseStream,
    StreamClosed,
    Shutdown,
    ShutdownComplete,
    // Data transfer
    SendFrame(Frame),
    FrameSent,
    ReceiveFrame,
    FrameReceived(Frame),
    // File transfer
    SendFile(ContentId, PathBuf),
    FileSent,
    ReceiveFile(ContentId, PathBuf),
    FileReceived(ContentId, PathBuf),
    // Progress
    Progress(u64),
    ProgressComplete,
}
