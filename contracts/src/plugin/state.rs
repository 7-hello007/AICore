use serde::{Deserialize, Serialize};

/// 插件生命周期状态。
///
/// 转换：
/// - Discovered → Validating
/// - Validating → Ready | Rejected
/// - Ready → Active | Disabled
/// - Active → Disabled | Failed
/// - Disabled → Active
/// - Failed → Disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Discovered,
    Validating,
    Ready,
    Active,
    Disabled,
    Rejected,
    Failed,
}

impl PluginState {
    /// 是否处于"可运行"状态。
    pub fn is_operational(self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否处于"终态"（不再自动转换）。
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// 是否允许转换到 `next`。
    pub fn can_transition_to(self, next: PluginState) -> bool {
        matches!(
            (self, next),
            (Self::Discovered, Self::Validating)
                | (Self::Validating, Self::Ready)
                | (Self::Validating, Self::Rejected)
                | (Self::Ready, Self::Active)
                | (Self::Ready, Self::Disabled)
                | (Self::Active, Self::Disabled)
                | (Self::Active, Self::Failed)
                | (Self::Disabled, Self::Active)
                | (Self::Failed, Self::Disabled)
        )
    }
}
