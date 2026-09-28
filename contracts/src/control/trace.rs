use serde::{Deserialize, Serialize};

use crate::execution::status::ExecutionOverallStatus;

use super::{rollback::RollbackRecord, stage::StageOutcome};

/// A complete track of transparent controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlTrace {
    pub trace_id: String,
    pub policy_id: String,
    /// Implementation results in six phases, in order of occurrence.
    pub stages: Vec<StageOutcome>,
    /// Final Implementation Status (Apply after).
    pub final_status: ExecutionOverallStatus,
    /// Revert records (not activated but `triggered=false`).
    pub rollback: RollbackRecord,
    /// Total time-consuming.
    pub total_duration_ms: u64,
}

impl ControlTrace {
    /// Whether the actual implementation was successful and there was no stage of failure (excluding Rollback).
    /// Neither NoOp nor the successful rollback is an effective strategy.
    pub fn is_success(&self) -> bool {
        self.final_status.is_success()
            && self
                .stages
                .iter()
                .filter(|s| s.stage != super::stage::ControlStage::Rollback)
                .all(|s| s.status.is_success())
    }

    /// Find some stage of results.
    pub fn stage(&self, stage: super::stage::ControlStage) -> Option<&StageOutcome> {
        self.stages.iter().find(|s| s.stage == stage)
    }
}
