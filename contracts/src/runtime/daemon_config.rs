use serde::{Deserialize, Serialize};

use super::mode::RuntimeMode;

/// 遥测采样配置。
///
/// V2-A-3 移交项：采样周期由 daemon 调度器使用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryConfig {
    /// 采样周期，毫秒。
    pub sampling_interval_ms: u64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 100,
        }
    }
}

impl TelemetryConfig {
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.sampling_interval_ms == 0 {
            return Err(AicoreError::ConfigInvalid(
                "sampling_interval_ms must be > 0".to_string(),
            ));
        }
        if self.sampling_interval_ms > 60_000 {
            return Err(AicoreError::ConfigInvalid(
                "sampling_interval_ms must be <= 60000".to_string(),
            ));
        }
        Ok(())
    }
}

/// Daemon 顶层配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonConfig {
    pub telemetry: TelemetryConfig,
    /// 每 N 次 tick 写一次状态文件。
    pub state_write_interval_ticks: u64,
    /// 优雅退出的最长等待时间，毫秒。
    pub shutdown_timeout_ms: u64,
    /// 启动时的初始 RuntimeMode。
    pub initial_mode: RuntimeMode,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            telemetry: TelemetryConfig::default(),
            state_write_interval_ticks: 10,
            shutdown_timeout_ms: 5_000,
            initial_mode: RuntimeMode::Normal,
        }
    }
}

impl DaemonConfig {
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        self.telemetry.validate()?;
        use crate::errors::AicoreError;
        if self.state_write_interval_ticks == 0 {
            return Err(AicoreError::ConfigInvalid(
                "state_write_interval_ticks must be > 0".to_string(),
            ));
        }
        if self.shutdown_timeout_ms == 0 {
            return Err(AicoreError::ConfigInvalid(
                "shutdown_timeout_ms must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}
