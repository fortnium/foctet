use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct LogConfig {
    pub log_level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            log_level: "info".to_string(),
        }
    }
}
