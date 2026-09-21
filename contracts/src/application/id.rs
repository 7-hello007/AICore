use serde::{Deserialize, Serialize};

/// 应用标识符。
///
/// 约定：反向域名风格，例如 "com.example.vscode"。
/// 不强制格式，但必须非空、长度 ≤ 128。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApplicationId(pub String);

impl ApplicationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 基本合法性校验。
    pub fn is_valid(&self) -> bool {
        let s = self.0.trim();
        !s.is_empty() && s.len() <= 128
    }
}

impl std::fmt::Display for ApplicationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
