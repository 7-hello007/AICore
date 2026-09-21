use serde::{Deserialize, Serialize};

/// 运行时模式。
///
/// - Normal：使用配置的主引擎（AI 辅助）。
/// - Degraded：跳过 AI，使用规则引擎。
/// - Safe：跳过 AI，使用规则引擎，并记录高频失败供审计。
/// - Recovery：连续成功后逐步恢复到 Normal，仅用于状态可观测性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    #[default]
    Normal,
    Degraded,
    Safe,
    /// 从 Safe/Degraded 向 Normal 恢复的过渡态。
    Recovery,
}
