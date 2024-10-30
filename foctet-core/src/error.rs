use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("Stream is closed")]
    Closed,
    #[error("Stream timed out")]
    Timeout,
    #[error("Stream encountered an unexpected error: {0}")]
    Unexpected(String),
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Connection is closed")]
    Closed,
    #[error("Connection timed out")]
    Timeout,
    #[error("Connection encountered an unexpected error: {0}")]
    Unexpected(String),
}
