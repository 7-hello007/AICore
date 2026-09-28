use serde::{Deserialize, Serialize};

use super::status::{ExecutionOverallStatus, ExecutionStatus};

/// Results of individual execution actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionOutcome {
    /// Resource categories, such as "power.profile"/ "process.priority".
    pub resource: String,
    /// Targets such as "system"/ "pid=1234".
    pub target: String,
    pub status: ExecutionStatus,
    pub message: String,
}

/// Complete results of a one-time strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionResult {
    pub policy_id: String,
    pub overall_status: ExecutionOverallStatus,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub outcomes: Vec<ExecutionOutcome>,
}

impl ExecutionResult {
    pub fn summarize(&self) -> String {
        format!(
            "policy={} status={:?} outcomes={}",
            self.policy_id,
            self.overall_status,
            self.outcomes.len()
        )
    }
}
