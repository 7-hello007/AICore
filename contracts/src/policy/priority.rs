use serde::{Deserialize, Serialize};

/// Strategy priority.
///
/// Semantic order: Background < Low < Nomal < High < Interactive.
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

/// Strategic source. Used for interpretability and auditing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    RuleBased,
    AiRecommended,
    UserSpecified,
    #[default]
    DefaultFallback,
}
