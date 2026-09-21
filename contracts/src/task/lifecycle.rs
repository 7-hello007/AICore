use serde::{Deserialize, Serialize};

/// Task 生命周期状态。
///
/// 转换：
/// - Queued → Running → Completed
/// - Queued → Running → Failed
/// - Queued → Cancelled
/// - Queued → Expired（deadline 超时）
/// - Running → Cancelled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Expired,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Expired
        )
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    pub fn can_transition_to(self, next: TaskState) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Running)
                | (Self::Queued, Self::Cancelled)
                | (Self::Queued, Self::Expired)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
        )
    }
}
