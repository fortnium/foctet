use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnixTimestamp(u64);

impl UnixTimestamp {
    /// Get the current time and return it as `UnixTimestamp`
    pub fn now() -> Self {
        let duration_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        UnixTimestamp(duration_since_epoch.as_secs())
    }

    /// Create `UnixTimestamp` from `SystemTime`
    pub fn from_system_time(time: SystemTime) -> Self {
        let duration_since_epoch = time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        UnixTimestamp(duration_since_epoch.as_secs())
    }

    /// Convert `UnixTimestamp` to `SystemTime`
    pub fn to_system_time(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.0)
    }

    /// Create `UnixTimestamp` from `u64` timestamp value
    pub fn from_u64(timestamp: u64) -> Self {
        UnixTimestamp(timestamp)
    }

    /// Convert `UnixTimestamp` to `u64`
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for UnixTimestamp {
    fn default() -> Self {
        UnixTimestamp(0)
    }
}
