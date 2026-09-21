use aicore_contracts::telemetry::npu::NpuSnapshot;

/// Windows NPU 探测。
///
/// Windows 没有统一的跨厂商 NPU telemetry ABI；当 vendor runtime 未接入时，
/// 返回明确的不可用状态而不是伪造利用率。
pub fn read_npu() -> NpuSnapshot {
    NpuSnapshot::default()
}
