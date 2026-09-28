use serde::{Deserialize, Serialize};

/// Apply identifiers.
///
/// Engagement: Reverse domain name style, such as "com.example.vscode".
/// Format is not mandatory, but must be non-empty, long ≤128.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApplicationId(pub String);

impl ApplicationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Basic legality verification.
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
