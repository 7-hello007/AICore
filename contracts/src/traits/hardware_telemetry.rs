use crate::errors::ContractResult;
use crate::hardware::telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample};

/// 硬件遥测提供者。
///
/// 实现：
/// - Linux：sysfs / DRM / vendor runtime
/// - Windows：DXGI / vendor SDK
/// - Mock：测试用
pub trait IHardwareTelemetryProvider: Send + Sync {
    /// 枚举 GPU 设备。
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo>;

    /// 枚举 NPU 设备。
    fn npu_devices(&self) -> Vec<HardwareDeviceInfo>;

    /// 采样指定 GPU。
    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample>;

    /// 采样指定 NPU。
    fn sample_npu(&self, device_id: &str) -> ContractResult<NpuTelemetrySample>;

    fn provider_name(&self) -> &'static str;
}
