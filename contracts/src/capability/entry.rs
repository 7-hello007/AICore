use serde::{Deserialize, Serialize};

use super::status::CapabilityStatus;

/// 单条能力描述。
///
/// 约定：
/// - `name` 使用点分小写，例如 `process.priority.write`。
/// - `detail` 在 `Partial` / `Conditional` / `Unsupported` 时**必须**非空，
///   说明限制或原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEntry {
    pub name: String,
    pub status: CapabilityStatus,
    pub detail: Option<String>,
}

impl CapabilityEntry {
    pub fn supported(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CapabilityStatus::Supported,
            detail: None,
        }
    }

    pub fn partial(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CapabilityStatus::Partial,
            detail: Some(detail.into()),
        }
    }

    pub fn read_only(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CapabilityStatus::ReadOnly,
            detail: None,
        }
    }

    pub fn conditional(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CapabilityStatus::Conditional,
            detail: Some(detail.into()),
        }
    }

    pub fn unsupported(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CapabilityStatus::Unsupported,
            detail: Some(detail.into()),
        }
    }

    /// 契约自检：非 Supported 状态必须带 detail。
    pub fn is_well_formed(&self) -> bool {
        match self.status {
            CapabilityStatus::Supported | CapabilityStatus::ReadOnly => true,
            CapabilityStatus::Partial
            | CapabilityStatus::Conditional
            | CapabilityStatus::Unsupported => self
                .detail
                .as_ref()
                .map(|d| !d.trim().is_empty())
                .unwrap_or(false),
        }
    }
}
