use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;
use crate::task::contract::TaskContract;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSubmitted {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub application_id: ApplicationId,
    pub contract: TaskContract,
    pub accepted: bool,
    pub message: String,
}
