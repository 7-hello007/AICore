use serde::{Deserialize, Serialize};

/// Plugin life cycle status.
///
/// Conversion:
/// - Discovered → Validating
/// - Validating → Ready | Rejected
/// - Ready → Active | Disabled
/// - Active → Disabled | Failed
/// - Disabled → Active
/// - Failed → Disabled
/// - Failed / Disabled → Validating (explicit pinned restart)
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
    /// Whether or not it's "runable".
    pub fn is_operational(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether it is in the "end" (no longer automatically converted).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Whether to allow conversion to `next`.
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
                | (Self::Disabled, Self::Validating)
                | (Self::Failed, Self::Disabled)
                | (Self::Failed, Self::Validating)
        )
    }
}
