use serde::{Deserialize, Serialize};

use super::mode::RuntimeMode;

/// Telemetry sample configuration.
///
/// V2-A-3 Transfer item: The sampling cycle is used by the daemon scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryConfig {
    /// Sample cycle, millisecond.
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

/// Daemon top layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonConfig {
    pub telemetry: TelemetryConfig,
    /// Writes a status file every N tick.
    pub state_write_interval_ticks: u64,
    /// The maximum waiting time for elegant exit, millisecond.
    pub shutdown_timeout_ms: u64,
    /// Initial RuntimeMode at startup.
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
