use serde::{Deserialize, Serialize};

/// AdaptiveLoop configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveLoopConfig {
    /// Could not close temporary folder: %s
    pub degraded_threshold: u32,
    /// Continuous failure to reach this value into Safe.
    pub safe_threshold: u32,
    /// Number of consecutive successes required before promotion.
    pub recovery_threshold: u32,
}

impl Default for AdaptiveLoopConfig {
    fn default() -> Self {
        Self {
            degraded_threshold: 2,
            safe_threshold: 5,
            recovery_threshold: 3,
        }
    }
}

impl AdaptiveLoopConfig {
    /// Validation of basic legality. Return None is legal.
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.degraded_threshold == 0 {
            return Err(AicoreError::ConfigInvalid(
                "degraded_threshold must be > 0".to_string(),
            ));
        }
        if self.safe_threshold < self.degraded_threshold {
            return Err(AicoreError::ConfigInvalid(
                "safe_threshold must be >= degraded_threshold".to_string(),
            ));
        }
        if self.recovery_threshold == 0 {
            return Err(AicoreError::ConfigInvalid(
                "recovery_threshold must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}
