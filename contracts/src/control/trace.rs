use serde::{Deserialize, Serialize};

use crate::execution::status::ExecutionOverallStatus;

use super::{rollback::RollbackRecord, stage::StageOutcome};

/// 一次透明控制的完整追踪。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlTrace {
    pub trace_id: String,
    pub policy_id: String,
    /// 六个阶段的执行结果，按发生顺序。
    pub stages: Vec<StageOutcome>,
    /// 最终执行状态（Apply 后）。
    pub final_status: ExecutionOverallStatus,
    /// 回退记录（未触发时存在但 `triggered=false`）。
    pub rollback: RollbackRecord,
    /// 总耗时。
    pub total_duration_ms: u64,
}

impl ControlTrace {
    /// 是否实际执行成功且没有失败阶段（不包括 Rollback）。
    /// NoOp 与成功回滚均不表示本次策略生效。
    pub fn is_success(&self) -> bool {
        self.final_status.is_success()
            && self
                .stages
                .iter()
                .filter(|s| s.stage != super::stage::ControlStage::Rollback)
                .all(|s| s.status.is_success())
    }

    /// 查找某阶段结果。
    pub fn stage(&self, stage: super::stage::ControlStage) -> Option<&StageOutcome> {
        self.stages.iter().find(|s| s.stage == stage)
    }
}
