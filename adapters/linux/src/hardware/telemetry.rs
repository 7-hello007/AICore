use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    hardware::telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample},
    traits::hardware_telemetry::IHardwareTelemetryProvider,
};

use crate::telemetry::sysfs;

const DRM_BASE: &str = "/sys/class/drm";
const ACCEL_BASE: &str = "/sys/class/accel";

/// Linux hardware telemetry provider.
///
/// - GPU：`/sys/class/drm/card*/device/`
/// - NPU：`/sys/class/accel/accel*`
pub struct LinuxHardwareTelemetryProvider;

impl LinuxHardwareTelemetryProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxHardwareTelemetryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareTelemetryProvider for LinuxHardwareTelemetryProvider {
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo> {
        let mut out = Vec::new();
        for card in sysfs::list_dir(DRM_BASE) {
            if !card.starts_with("card") || card.contains('-') {
                continue;
            }
            let device_path = format!("{}/{}/device", DRM_BASE, card);
            if !Path::new(&device_path).exists() {
                continue;
            }
            let vendor_id = sysfs::read_trimmed(format!("{}/vendor", device_path));
            let vendor =
                vendor_from_id(vendor_id.as_deref()).unwrap_or_else(|| "Unknown".to_string());

            let driver = std::fs::read_link(format!("{}/driver", device_path))
                .ok()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()));

            out.push(HardwareDeviceInfo {
                id: format!("drm:{}", card),
                vendor,
                name: card.clone(),
                driver,
            });
        }
        out
    }

    fn npu_devices(&self) -> Vec<HardwareDeviceInfo> {
        let mut out = Vec::new();
        for entry in sysfs::list_dir(ACCEL_BASE) {
            if !entry.starts_with("accel") {
                continue;
            }
            out.push(HardwareDeviceInfo {
                id: format!("npu:{}", entry),
                vendor: "Unknown".to_string(),
                name: entry.clone(),
                driver: None,
            });
        }
        out
    }

    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample> {
        let card = device_id
            .strip_prefix("drm:")
            .ok_or_else(|| ContractError::InvalidRequest(format!("bad gpu id: {}", device_id)))?;

        let device_path = format!("{}/{}/device", DRM_BASE, card);
        if !Path::new(&device_path).exists() {
            return Err(ContractError::ResourceUnavailable(format!(
                "gpu device not found: {}",
                device_id
            )));
        }

        let vendor_id = sysfs::read_trimmed(format!("{}/vendor", device_path));
        let vendor = vendor_from_id(vendor_id.as_deref()).unwrap_or_else(|| "Unknown".to_string());

        let utilization = sysfs::read_f32(format!("{}/gpu_busy_percent", device_path));
        let memory_total_mb = sysfs::read_u64(format!("{}/mem_info_vram_total", device_path))
            .map(|v| v / 1024 / 1024);
        let memory_used_mb =
            sysfs::read_u64(format!("{}/mem_info_vram_used", device_path)).map(|v| v / 1024 / 1024);
        let temperature_celsius = read_hwmon_temp(&device_path);

        Ok(GpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor,
                name: card.to_string(),
                driver: None,
            },
            utilization_percent: utilization,
            memory_used_mb,
            memory_total_mb,
            temperature_celsius,
            sampled_at_unix_ms: now_unix_ms(),
        })
    }

    fn sample_npu(&self, device_id: &str) -> ContractResult<NpuTelemetrySample> {
        let entry = device_id
            .strip_prefix("npu:")
            .ok_or_else(|| ContractError::InvalidRequest(format!("bad npu id: {}", device_id)))?;

        let path = format!("{}/{}", ACCEL_BASE, entry);
        if !Path::new(&path).exists() {
            return Err(ContractError::ResourceUnavailable(format!(
                "npu device not found: {}",
                device_id
            )));
        }

        // There is no universal NPU access interface on Linux; do your best.
        let utilization = sysfs::read_f32(format!("{}/device/utilization", path));

        Ok(NpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor: "Unknown".to_string(),
                name: entry.to_string(),
                driver: None,
            },
            utilization_percent: utilization,
            sampled_at_unix_ms: now_unix_ms(),
        })
    }

    fn provider_name(&self) -> &'static str {
        "linux_hardware_telemetry"
    }
}

fn read_hwmon_temp(device_path: &str) -> Option<f32> {
    let hwmon_base = format!("{}/hwmon", device_path);
    for hw in sysfs::list_dir(&hwmon_base) {
        if !hw.starts_with("hwmon") {
            continue;
        }
        let temp_path = format!("{}/{}/temp1_input", hwmon_base, hw);
        if let Some(milli) = sysfs::read_i64(&temp_path) {
            return Some(milli as f32 / 1000.0);
        }
    }
    None
}

fn vendor_from_id(id: Option<&str>) -> Option<String> {
    let id = id?.trim().to_lowercase();
    let normalized = id.trim_start_matches("0x");
    match normalized {
        "1002" => Some("AMD".to_string()),
        "10de" => Some("NVIDIA".to_string()),
        "8086" => Some("Intel".to_string()),
        _ => Some(format!("Unknown (0x{})", id)),
    }
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
