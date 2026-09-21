use serde::{Deserialize, Serialize};

/// Tier 唯一标识。
///
/// 约定：小写点分，例如 `l0.legacy` / `l1.aware` / `l2.native`。
/// Core **不得**根据 TierId 分支；TierId 只用于日志与诊断。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TierId(pub String);

impl TierId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        let s = self.0.trim();
        !s.is_empty() && s.len() <= 128 && !s.contains(char::is_whitespace)
    }
}

impl std::fmt::Display for TierId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
