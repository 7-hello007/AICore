use aicore_contracts::telemetry::ComputerSnapshot;

/// Core internal computer state.
///
/// Different from ComputerSnapshot:
/// - Snapshot is the harvest layer output.
/// - ComputerState is a standardized field model for use by Context / Policy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComputerState {
    pub snapshot: ComputerSnapshot,
    pub health: SystemHealth,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemHealth {
    pub cpu_available: bool,
    pub gpu_available: bool,
    pub npu_available: bool,
    pub battery_present: bool,
}
