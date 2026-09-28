use crate::errors::ContractResult;
use serde::{Deserialize, Serialize};

/// Hardware execution hint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HardwareExecutionHint {
    /// Workload preference, such as "graphics" / "compute" / "auto".
    WorkloadPreference(String),
    /// Maximum power in watts (best effort).
    MaxPowerWatts(f32),
    /// Graphics-only mode.
    GraphicsMode,
    /// Compute-only mode.
    ComputeMode,
    /// Automatic mode.
    AutoMode,
}

/// Hardware execution status.
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareExecutionStatus {
    /// Applied.
    Applied,
    /// Already in target status (no change).
    AlreadyInState,
    /// Skipped (e.g. capability available but user disabled).
    Skipped { reason: String },
    /// The platform is not supported.
    NotSupported { reason: String },
    /// Execution failed.
    Failed { reason: String },
}

impl HardwareExecutionStatus {
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied | Self::AlreadyInState)
    }
}

/// Hardware execution request.
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareExecutionRequest {
    pub device_id: String,
    pub hint: HardwareExecutionHint,
}

/// Hardware execution result.
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareExecutionResult {
    pub device_id: String,
    pub status: HardwareExecutionStatus,
    pub latency_ms: u64,
}

/// Hardware executor.
///
/// Duties:
/// - Apply `HardwareExecutionHint` to specific devices.
/// - Report failures through `HardwareExecutionStatus` without panicking.
/// - The Binder calls this interface; it does not call the Executor directly.
///
/// Platform:
/// - Linux: best-effort, capability-gated sysfs or vendor-runtime writes.
/// - Windows: best-effort vendor control (for example NVIDIA); other paths return `NotSupported`.
///
/// `NotSupported` means the capability is unavailable and must not be reported as success.
pub trait IHardwareExecutor: Send + Sync {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult>;

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult>;

    fn executor_name(&self) -> &'static str;
}
