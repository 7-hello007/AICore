use serde::{Deserialize, Serialize};

/// AdaptiveLoop 配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveLoopConfig {
    /// 连续失败达到此值进入 Degraded。
    pub degraded_threshold: u32,
    /// 连续失败达到此值进入 Safe。
    pub safe_threshold: u32,
    /// 连续成功达到此值回升一级。
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
    /// 基本合法性校验。返回 None 表示合法。
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
