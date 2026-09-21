use serde::{Deserialize, Serialize};

use crate::telemetry::ComputerSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStateUpdated {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub snapshot: ComputerSnapshot,
}
