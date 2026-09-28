use aicore_contracts::telemetry::npu::NpuSnapshot;

/// Windows NPU detection.
///
/// Windows does not have a single cross-vendor, NPU telemetry ABI; when vendor runtime is not connected, it is not possible to find a single multi-producer.
/// Returns a clear state of non-availability rather than falsifying utilization.
pub fn read_npu() -> NpuSnapshot {
    NpuSnapshot::default()
}
