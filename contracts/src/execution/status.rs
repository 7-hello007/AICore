use serde::{Deserialize, Serialize};

/// 单个执行动作的结果状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Success,
    Skipped,
    NotSupported,
    PermissionDenied,
    Failed,
}

impl ExecutionStatus {
    /// 是否算作"真失败"（喂给 SafeMode）。
    ///
    /// - `Failed` / `PermissionDenied`：真失败。
    /// - `NotSupported`：能力缺失，不算失败（应走 capability 降级）。
    /// - `Skipped`：显式跳过，不算失败。
    pub fn is_real_failure(self) -> bool {
        matches!(self, Self::Failed | Self::PermissionDenied)
    }
}

/// 整体执行结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOverallStatus {
    AllSucceeded,
    PartiallySucceeded,
    /// 所有 outcome 都是 Skipped / NotSupported，没有任何实际操作。
    NoOp,
    AllFailed,
}

impl ExecutionOverallStatus {
    pub fn from_outcomes(outcomes: &[super::result::ExecutionOutcome]) -> Self {
        if outcomes.is_empty() {
            return Self::NoOp;
        }

        let total = outcomes.len();
        let success = outcomes
            .iter()
            .filter(|o| o.status == ExecutionStatus::Success)
            .count();
        let real_failures = outcomes
            .iter()
            .filter(|o| o.status.is_real_failure())
            .count();

        if success == total {
            return Self::AllSucceeded;
        }
        if success > 0 {
            // 有成功、有非成功（NotSupported / Skipped / Failed）→ 部分成功
            return Self::PartiallySucceeded;
        }
        if real_failures > 0 {
            // 无成功、有真失败 → 全部失败
            return Self::AllFailed;
        }
        // 无成功、无真失败 → 全是 NotSupported / Skipped → NoOp
        Self::NoOp
    }

    /// 是否算作"对 SafeMode 友好的成功"。
    ///
    /// - AllSucceeded / PartiallySucceeded → true（成功，重置失败计数）
    /// - NoOp → false（不改变 SafeMode 状态）
    /// - AllFailed → false
    pub fn is_success(self) -> bool {
        matches!(self, Self::AllSucceeded | Self::PartiallySucceeded)
    }

    /// 是否算作"真失败"（喂给 SafeMode）。
    pub fn is_failure(self) -> bool {
        matches!(self, Self::AllFailed)
    }
}
