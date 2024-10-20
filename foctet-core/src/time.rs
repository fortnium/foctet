use std::time::{SystemTime, UNIX_EPOCH, Duration};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnixTimestamp(u64);

impl UnixTimestamp {
    /// 現在時刻を取得して`UnixTimestamp`として返す
    pub fn now() -> Self {
        let duration_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        UnixTimestamp(duration_since_epoch.as_secs())
    }

    /// `SystemTime` から `UnixTimestamp` を作成
    pub fn from_system_time(time: SystemTime) -> Self {
        let duration_since_epoch = time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        UnixTimestamp(duration_since_epoch.as_secs())
    }

    /// `UnixTimestamp` から `SystemTime` に変換
    pub fn to_system_time(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.0)
    }

    /// `u64` のタイムスタンプ値から `UnixTimestamp` を作成
    pub fn from_u64(timestamp: u64) -> Self {
        UnixTimestamp(timestamp)
    }

    /// `UnixTimestamp` を `u64` に変換
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for UnixTimestamp {
    fn default() -> Self {
        UnixTimestamp(0)
    }
}
