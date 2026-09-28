use serde::{Deserialize, Serialize};

/// Hardware device identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareDeviceInfo {
    /// Steady ID, such as "drum:card0"/ "npu:accel0".
    pub id: String,
    /// Vendors such as "AMD"/ "NVIDIA"/ "Intel"
    pub vendor: String,
    /// Device name.
    pub name: String,
    /// Driver name (if readable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

/// GPU telemetry sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuTelemetrySample {
    pub device: HardwareDeviceInfo,
    pub utilization_percent: Option<f32>,
    pub memory_used_mb: Option<u64>,
    pub memory_total_mb: Option<u64>,
    pub temperature_celsius: Option<f32>,
    pub sampled_at_unix_ms: u64,
}

/// NPU telemetry sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpuTelemetrySample {
    pub device: HardwareDeviceInfo,
    pub utilization_percent: Option<f32>,
    pub sampled_at_unix_ms: u64,
}
