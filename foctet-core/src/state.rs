#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug)]
pub enum StreamState {
    Open,
    Closed,
}
