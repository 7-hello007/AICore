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
    /// The backend accepted/applied the operation. This alone is not independent
    /// verification and may describe an in-memory model.
    Applied,
    /// A guarded real-hardware transaction independently read back the target.
    /// This proves observed state, not a workload/performance benefit.
    Verified,
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
        matches!(self, Self::Applied | Self::AlreadyInState | Self::Verified)
    }

    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
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

/// Independently observed state. `None` preserves the absence of a model hint;
/// it must not be replaced by an invented `auto` value during restoration.
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareStateSnapshot {
    pub device_id: String,
    pub hint: Option<HardwareExecutionHint>,
}

/// Hardware executor.
///
/// Duties:
/// - Apply `HardwareExecutionHint` to specific devices.
/// - Report failures through `HardwareExecutionStatus` without panicking.
/// - The control layer binds this backend behind a safe actuator. Runtime
///   routing must never call its mutation methods without that authorization.
///
/// Platform:
/// - Native adapters without exact reversible state return `NotSupported`
///   before mutation; command availability is insufficient proof of support.
///
/// `NotSupported` means the capability is unavailable and must not be reported as success.
pub trait IHardwareExecutor: Send + Sync {
    /// Model backends cannot be authorized with a real-hardware permit.
    fn is_model_only(&self) -> bool {
        false
    }

    /// True only for a backend that implements exact snapshot restoration.
    fn supports_reversible_state(&self) -> bool {
        false
    }

    /// Query the actual device state independently of command acknowledgements.
    /// Unsupported backends must reject before any mutation.
    fn read_hardware_state(&self, _device_id: &str) -> ContractResult<HardwareStateSnapshot> {
        Err(crate::errors::ContractError::NotSupported(
            "independent reversible hardware state unavailable".into(),
        ))
    }

    /// Restore the exact captured state and independently verify it.
    fn restore_hardware_state(
        &self,
        _state: &HardwareStateSnapshot,
    ) -> ContractResult<HardwareExecutionResult> {
        Err(crate::errors::ContractError::NotSupported(
            "exact hardware state restoration unavailable".into(),
        ))
    }

    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult>;

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult>;

    fn executor_name(&self) -> &'static str;
}
