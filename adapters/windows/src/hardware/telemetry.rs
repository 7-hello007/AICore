use aicore_contracts::{
    errors::{ContractError, ContractResult},
    hardware::telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample},
    traits::hardware_telemetry::IHardwareTelemetryProvider,
};
use std::process::Command;

pub struct WindowsHardwareTelemetryProvider;
impl WindowsHardwareTelemetryProvider {
    pub fn new() -> Self {
        Self
    }
    fn query(args: &[&str]) -> Option<String> {
        Command::new("nvidia-smi")
            .args(args)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
    }
}
impl Default for WindowsHardwareTelemetryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareTelemetryProvider for WindowsHardwareTelemetryProvider {
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo> {
        let Some(out) = Self::query(&[
            "--query-gpu=index,name,driver_version",
            "--format=csv,noheader,nounits",
        ]) else {
            return Vec::new();
        };
        out.lines()
            .filter_map(|line| {
                let parts: Vec<_> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() < 3 {
                    return None;
                }
                Some(HardwareDeviceInfo {
                    id: format!("nvidia:{}", parts[0]),
                    vendor: "NVIDIA".to_string(),
                    name: parts[1].to_string(),
                    driver: Some(parts[2].to_string()),
                })
            })
            .collect()
    }
    fn npu_devices(&self) -> Vec<HardwareDeviceInfo> {
        Vec::new()
    }
    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample> {
        let index = device_id.strip_prefix("nvidia:").ok_or_else(|| {
            ContractError::InvalidRequest(format!("unsupported gpu id: {}", device_id))
        })?;
        let out = Self::query(&["-i", index, "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,name,driver_version", "--format=csv,noheader,nounits"])
            .ok_or_else(|| ContractError::NotSupported("nvidia-smi unavailable or query failed".to_string()))?;
        let p: Vec<_> = out.split(',').map(|s| s.trim()).collect();
        if p.len() < 6 {
            return Err(ContractError::AdapterUnavailable(
                "unexpected nvidia-smi output".to_string(),
            ));
        }
        let parse_f32 = |s: &str| s.parse::<f32>().ok();
        let parse_u64 = |s: &str| s.parse::<u64>().ok();
        Ok(GpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor: "NVIDIA".to_string(),
                name: p[4].to_string(),
                driver: Some(p[5].to_string()),
            },
            utilization_percent: parse_f32(p[0]),
            memory_used_mb: parse_u64(p[1]),
            memory_total_mb: parse_u64(p[2]),
            temperature_celsius: parse_f32(p[3]),
            sampled_at_unix_ms: now_unix_ms(),
        })
    }
    fn sample_npu(&self, _device_id: &str) -> ContractResult<NpuTelemetrySample> {
        Err(ContractError::NotSupported(
            "Windows NPU telemetry requires Windows ML/vendor runtime adapter".to_string(),
        ))
    }
    fn provider_name(&self) -> &'static str {
        "windows_hardware_telemetry"
    }
}
fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
