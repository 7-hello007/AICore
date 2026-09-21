use serde::{Deserialize, Serialize};

/// 回退记录。
///
/// 每次控制尝试失败并回退时生成；用于审计与追踪。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackRecord {
    /// 是否触发了回退。
    pub triggered: bool,
    /// 回退前生效的 policy id（None 表示无前一条）。
    pub from_policy_id: Option<String>,
    /// 回退目标 policy id（None 表示无可用回退目标）。
    pub to_policy_id: Option<String>,
    /// 回退原因。
    pub reason: Option<String>,
    /// 回退是否成功。
    pub succeeded: Option<bool>,
}

impl RollbackRecord {
    pub fn not_triggered() -> Self {
        Self {
            triggered: false,
            from_policy_id: None,
            to_policy_id: None,
            reason: None,
            succeeded: None,
        }
    }

    pub fn triggered(
        from: Option<String>,
        to: Option<String>,
        reason: impl Into<String>,
        succeeded: bool,
    ) -> Self {
        Self {
            triggered: true,
            from_policy_id: from,
            to_policy_id: to,
            reason: Some(reason.into()),
            succeeded: Some(succeeded),
        }
    }
}

impl Default for RollbackRecord {
    fn default() -> Self {
        Self::not_triggered()
    }
}
