use serde::{Deserialize, Serialize};

use crate::runtime::mode::RuntimeMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeModeChanged {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub from: RuntimeMode,
    pub to: RuntimeMode,
    pub reason: String,
}
