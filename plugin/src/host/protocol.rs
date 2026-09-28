use aicore_contracts::hardware::capability::HardwareDeviceKind;
use aicore_contracts::policy::priority::PolicyPriority;
use aicore_contracts::policy::resource_policy::PowerMode;
use aicore_contracts::telemetry::snapshot::ComputerSnapshot;
use aicore_contracts::traits::hardware_executor::HardwareExecutionHint;
use serde::{Deserialize, Serialize};

/// Plugin process IPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostRequest {
    /// Ping Detective.
    Ping,
    /// Execute plugin logic.
    Execute {
        input: u32,
    },
    /// Typed, read-only telemetry snapshot capability.
    ReadTelemetry,
    /// Typed process priority operation; activation checks the exact pin.
    SetPriority {
        pid: u32,
        priority: PolicyPriority,
    },
    /// Typed control. A host acknowledgement alone is never readback proof.
    SetHardware {
        device_kind: HardwareDeviceKind,
        device_id: String,
        hint: HardwareExecutionHint,
    },
    SetPower {
        mode: PowerMode,
    },
    /// Request closed.
    Shutdown,
}

/// Plugin process IPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostResponse {
    Pong,
    Result {
        output: u32,
    },
    Telemetry {
        snapshot: Box<ComputerSnapshot>,
    },
    PrioritySet {
        pid: u32,
        priority: PolicyPriority,
    },
    HardwareSet {
        device_kind: HardwareDeviceKind,
        device_id: String,
        hint: HardwareExecutionHint,
    },
    PowerSet {
        mode: PowerMode,
    },
    ShuttingDown,
    Error {
        message: String,
    },
}

pub const MAX_LINE_BYTES: usize = 64 * 1024;
