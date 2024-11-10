use foctet::core::default::{DEFAULT_READ_BUFFER_SIZE, DEFAULT_WRITE_BUFFER_SIZE};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct BufferConfig {
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_buffer_size: DEFAULT_WRITE_BUFFER_SIZE,
        }
    }
}
