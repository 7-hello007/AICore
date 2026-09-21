use serde::{Deserialize, Serialize};

/// 策略优先级。
///
/// 顺序即语义：Background < Low < Normal < High < Interactive。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyPriority {
    Background,
    Low,
    #[default]
    Normal,
    High,
    Interactive,
}

/// 策略来源。用于可解释性与审计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    RuleBased,
    AiRecommended,
    UserSpecified,
    #[default]
    DefaultFallback,
}
