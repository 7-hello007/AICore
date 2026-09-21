use serde::{Deserialize, Serialize};

use super::status::{ExecutionOverallStatus, ExecutionStatus};

/// 单个执行动作的结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionOutcome {
    /// 资源类别，例如 "power.profile" / "process.priority"。
    pub resource: String,
    /// 目标，例如 "system" / "pid=1234"。
    pub target: String,
    pub status: ExecutionStatus,
    pub message: String,
}

/// 一次策略执行的完整结果。
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
