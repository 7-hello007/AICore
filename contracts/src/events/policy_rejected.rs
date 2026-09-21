use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRejected {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub policy_id: String,
    pub issues: Vec<RejectionIssue>,
}
