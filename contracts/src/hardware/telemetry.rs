use serde::{Deserialize, Serialize};

/// 硬件设备身份。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareDeviceInfo {
    /// 稳定 ID，例如 "drm:card0" / "npu:accel0"。
    pub id: String,
    /// 厂商，例如 "AMD" / "NVIDIA" / "Intel"。
    pub vendor: String,
    /// 设备名。
    pub name: String,
    /// 驱动名（若可读）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

/// GPU 遥测样本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuTelemetrySample {
    pub device: HardwareDeviceInfo,
    pub utilization_percent: Option<f32>,
    pub memory_used_mb: Option<u64>,
    pub memory_total_mb: Option<u64>,
    pub temperature_celsius: Option<f32>,
    pub sampled_at_unix_ms: u64,
}

/// NPU 遥测样本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpuTelemetrySample {
    pub device: HardwareDeviceInfo,
    pub utilization_percent: Option<f32>,
    pub sampled_at_unix_ms: u64,
}
