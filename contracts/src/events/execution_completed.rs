use serde::{Deserialize, Serialize};

use crate::execution::result::ExecutionResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCompleted {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub result: ExecutionResult,
}
