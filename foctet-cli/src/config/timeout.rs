use foctet::core::default::{
    DEFAULT_CONNECTION_TIMEOUT, DEFAULT_RECEIVE_TIMEOUT, DEFAULT_SEND_TIMEOUT,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeoutConfig {
    pub conn_timeout: u64,
    pub read_timeout: u64,
    pub write_timeout: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            conn_timeout: DEFAULT_CONNECTION_TIMEOUT.as_secs(),
            read_timeout: DEFAULT_RECEIVE_TIMEOUT.as_secs(),
            write_timeout: DEFAULT_SEND_TIMEOUT.as_secs(),
        }
    }
}
