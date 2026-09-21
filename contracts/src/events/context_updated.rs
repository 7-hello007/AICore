use serde::{Deserialize, Serialize};

use crate::context::ContextSnapshot;

/// 当 Context Engine 产生新的上下文时发布。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUpdated {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub context: ContextSnapshot,
}
