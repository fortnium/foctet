use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StreamId(pub u32);

impl StreamId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    pub fn next(&self) -> Self {
        StreamId(self.0.checked_add(1).unwrap_or(0))
    }
    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&mut self, n: u32) -> Self {
        let old = self.0;
        self.0 = self.0.checked_add(n).unwrap_or(0);
        StreamId(old)
    }
}

impl nohash_hasher::IsEnabled for StreamId {}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub u32);

impl OperationId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    pub fn next(&self) -> Self {
        OperationId(self.0.checked_add(1).unwrap_or(0))
    }
    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&mut self, n: u32) -> Self {
        let old = self.0;
        self.0 = self.0.checked_add(n).unwrap_or(0);
        OperationId(old)
    }
}
