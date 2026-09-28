use crate::errors::ContractResult;
use crate::hardware::telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample};

/// Hardware telemetry provider.
///
/// Implementations provide:
/// - Linux：sysfs / DRM / vendor runtime
/// - Windows：DXGI / vendor SDK
/// -Mock: Test.
pub trait IHardwareTelemetryProvider: Send + Sync {
    /// Element GPU device.
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo>;

    /// Element NPU device.
    fn npu_devices(&self) -> Vec<HardwareDeviceInfo>;

    /// Sample specifies GPU.
    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample>;

    /// Sample specifies NPU.
    fn sample_npu(&self, device_id: &str) -> ContractResult<NpuTelemetrySample>;

    fn provider_name(&self) -> &'static str;
}
