use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    hardware::matrix::HardwareVendor,
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
    pub fn vendor_for_device(&self, device_id: &str) -> HardwareVendor {
        let path = gpu_device_path(device_id).or_else(|| npu_device_path(device_id));
        let numeric = path
            .and_then(|path| sysfs::read_trimmed(path.join("vendor")))
            .and_then(|text| u32::from_str_radix(text.trim_start_matches("0x"), 16).ok());
        numeric
            .map(HardwareVendor::from_pci_vendor_id)
            .unwrap_or_else(|| HardwareVendor::Other("unverified identity".into()))
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
                id: stable_device_id("drm", Path::new(&device_path)).unwrap_or_default(),
                vendor,
                name: card.clone(),
                driver,
            });
        }
        out.retain(|device| !device.id.is_empty());
        out
    }

    fn npu_devices(&self) -> Vec<HardwareDeviceInfo> {
        let mut out = Vec::new();
        for entry in sysfs::list_dir(ACCEL_BASE) {
            if !entry.starts_with("accel") {
                continue;
            }
            let device_path = Path::new(ACCEL_BASE).join(&entry).join("device");
            let Some(id) = stable_device_id("npu", &device_path) else {
                continue;
            };
            out.push(HardwareDeviceInfo {
                id,
                vendor: "Unknown".to_string(),
                name: entry.clone(),
                driver: None,
            });
        }
        out
    }

    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample> {
        let device_path = gpu_device_path(device_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!("gpu device not found: {device_id}"))
        })?;
        let device_path = device_path.to_string_lossy().to_string();
        let vendor_id = sysfs::read_trimmed(format!("{}/vendor", device_path));
        let vendor = vendor_from_id(vendor_id.as_deref()).unwrap_or_else(|| "Unknown".to_string());

        let utilization = sysfs::read_f32(format!("{}/gpu_busy_percent", device_path))
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value));
        let memory_total_mb = sysfs::read_u64(format!("{}/mem_info_vram_total", device_path))
            .map(|v| v / 1024 / 1024);
        let memory_used_mb =
            sysfs::read_u64(format!("{}/mem_info_vram_used", device_path)).map(|v| v / 1024 / 1024);
        let temperature_celsius = read_hwmon_temp(&device_path).filter(|value| value.is_finite());
        if utilization.is_none()
            && memory_total_mb.is_none()
            && memory_used_mb.is_none()
            && temperature_celsius.is_none()
        {
            return Err(ContractError::NotSupported(
                "GPU detected without readable telemetry".into(),
            ));
        }

        Ok(GpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor,
                name: device_id.to_string(),
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
        let path = npu_device_path(device_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!("npu device not found: {device_id}"))
        })?;
        // There is no universal NPU access interface on Linux; do your best.
        let utilization = sysfs::read_f32(path.join("utilization"))
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
            .ok_or_else(|| {
                ContractError::NotSupported(
                    "NPU detected without readable utilization telemetry".into(),
                )
            })?;

        Ok(NpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor: "Unknown".to_string(),
                name: device_id.to_string(),
                driver: None,
            },
            utilization_percent: Some(utilization),
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

// Resolve IDs only by enumerating actual OS records. Never use caller text as a path.
fn stable_device_id(kind: &str, device_path: &Path) -> Option<String> {
    device_path
        .canonicalize()
        .ok()
        .map(|path| format!("{kind}:sysfs:{}", path.to_string_lossy()))
}
pub(super) fn gpu_device_path(device_id: &str) -> Option<std::path::PathBuf> {
    discovered_device_path(DRM_BASE, "card", "drm", device_id)
}
pub(super) fn npu_device_path(device_id: &str) -> Option<std::path::PathBuf> {
    discovered_device_path(ACCEL_BASE, "accel", "npu", device_id)
}
fn discovered_device_path(
    base: &str,
    prefix: &str,
    kind: &str,
    device_id: &str,
) -> Option<std::path::PathBuf> {
    sysfs::list_dir(base)
        .into_iter()
        .filter(|entry| {
            entry
                .strip_prefix(prefix)
                .map(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
                .unwrap_or(false)
        })
        .find_map(|entry| {
            let path = Path::new(base).join(entry).join("device");
            if stable_device_id(kind, &path).as_deref() == Some(device_id) {
                Some(path)
            } else {
                None
            }
        })
}
