use serde::{Deserialize, Serialize};

use super::{capability::TierCapability, id::TierId, level::TierLevel};

/// Tier Registration Information.
///
/// Declared by Tier; Registry only performs validation and indexing and does not make business decisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierRegistration {
    pub id: TierId,
    pub level: TierLevel,
    pub capabilities: Vec<TierCapability>,
    /// Humans can read descriptions.
    #[serde(default)]
    pub description: String,
    /// Tier version, for example "1.0.0".
    pub version: String,
}

impl TierRegistration {
    pub fn new(id: TierId, level: TierLevel, version: impl Into<String>) -> Self {
        Self {
            id,
            level,
            capabilities: Vec::new(),
            description: String::new(),
            version: version.into(),
        }
    }

    pub fn with_capability(mut self, c: TierCapability) -> Self {
        if !self.capabilities.contains(&c) {
            self.capabilities.push(c);
        }
        self
    }

    pub fn with_capabilities(mut self, cs: Vec<TierCapability>) -> Self {
        for c in cs {
            self = self.with_capability(c);
        }
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn has_capability(&self, c: TierCapability) -> bool {
        self.capabilities.contains(&c)
    }

    /// Verify:
    /// - id legal.
    /// - version is empty.
    /// - At least declare a Capability.
    /// - L0 cannot declare NativeTaskControl.
    pub fn validate(&self) -> Result<(), crate::errors::AicoreError> {
        use crate::errors::AicoreError;

        if !self.id.is_valid() {
            return Err(AicoreError::TierInvalid(format!(
                "invalid tier id: {}",
                self.id.0
            )));
        }
        if self.version.trim().is_empty() {
            return Err(AicoreError::TierInvalid(
                "tier version is empty".to_string(),
            ));
        }
        if self.capabilities.is_empty() {
            return Err(AicoreError::TierInvalid(
                "tier declares no capabilities".to_string(),
            ));
        }
        if matches!(self.level, TierLevel::L0 | TierLevel::L1)
            && self.has_capability(TierCapability::NativeTaskControl)
        {
            return Err(AicoreError::TierLevelCannotControlNative(self.level));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TierRegistrationError {
    #[error("invalid tier id: {0}")]
    InvalidId(String),
    #[error("tier version is empty")]
    EmptyVersion,
    #[error("tier declares no capabilities")]
    NoCapabilities,
    #[error("{0:?} tier cannot declare NativeTaskControl")]
    LevelCannotControlNative(TierLevel),
}
