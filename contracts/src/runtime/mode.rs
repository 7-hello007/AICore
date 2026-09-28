use serde::{Deserialize, Serialize};

/// Run-time mode.
///
/// - Normal: Use the configured main engine (AI support).
/// - Degraded: Skip AI, use the rule engine.
/// - Safe: Skip AI, use a rule engine and record high frequency failures for audit.
/// Recovery: Gradual recovery to Normal after successive success, only for state-observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    #[default]
    Normal,
    Degraded,
    Safe,
    /// A transition from Safe/Degraded to Normal.
    Recovery,
}
