use aicore_contracts::telemetry::ComputerSnapshot;

/// Core 内部使用的计算机状态。
///
/// 与 ComputerSnapshot 的区别：
/// - Snapshot 是采集层输出。
/// - ComputerState 是规范化后、供 Context / Policy 使用的领域模型。
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
