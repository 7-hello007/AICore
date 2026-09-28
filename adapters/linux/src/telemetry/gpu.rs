use aicore_contracts::telemetry::gpu::GpuSnapshot;

use super::sysfs;

const DRM_BASE: &str = "/sys/class/drm";

/// Read GPU telemetry.
///
/// Support path:
/// - AMD (amdgpu): `/sys/class/drm/card*/device/gpu_busy_percent`
/// VRAM: `/sys/class/drm/card*/device/mem_info_vram_used` and `_total`
/// - Temperature: `/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input`
///
/// If no GPU is found, or a field cannot be read, return `None` values without panicking.
pub fn read_gpu() -> GpuSnapshot {
    let mut snapshot = GpuSnapshot::default();

    let cards = sysfs::list_dir(DRM_BASE);
    for card in cards {
        if !card.starts_with("card") || card.contains('-') {
            // Skip a connector like "card0-DP-1"
            continue;
        }

        let device_path = format!("{}/{}/device", DRM_BASE, card);
        if !std::path::Path::new(&device_path).exists() {
            continue;
        }

        // Read
        let vendor_id = sysfs::read_trimmed(format!("{}/vendor", device_path));
        let device_id = sysfs::read_trimmed(format!("{}/device", device_path));

        // vendor extrapolation
        if snapshot.vendor.is_none() {
            snapshot.vendor = vendor_from_id(vendor_id.as_deref());
        }

        // Name: Synthesize with vendor + data id to avoid empty
        if snapshot.name.is_none() {
            snapshot.name = Some(format!(
                "{} ({})",
                snapshot.vendor.clone().unwrap_or_else(|| "GPU".to_string()),
                device_id.clone().unwrap_or_else(|| "unknown".to_string())
            ));
        }

        // Usage
        if snapshot.usage_percent.is_none() {
            if let Some(v) = sysfs::read_f32(format!("{}/gpu_busy_percent", device_path)) {
                snapshot.usage_percent = Some(v);
            }
        }

        // VRAM
        if snapshot.memory_total_mb.is_none() {
            if let Some(total) = sysfs::read_u64(format!("{}/mem_info_vram_total", device_path)) {
                snapshot.memory_total_mb = Some(total / 1024 / 1024);
            }
        }
        if snapshot.memory_used_mb.is_none() {
            if let Some(used) = sysfs::read_u64(format!("{}/mem_info_vram_used", device_path)) {
                snapshot.memory_used_mb = Some(used / 1024 / 1024);
            }
        }

        // Temperature: Through hwmon
        if snapshot.temperature_celsius.is_none() {
            snapshot.temperature_celsius = read_hwmon_temp(&device_path);
        }
    }

    snapshot
}

fn read_hwmon_temp(device_path: &str) -> Option<f32> {
    let hwmon_base = format!("{}/hwmon", device_path);
    let hwmons = sysfs::list_dir(&hwmon_base);
    for hw in hwmons {
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
    let normalized = id.trim_start_matches("0x").trim_start_matches('0');
    match normalized {
        "1002" => Some("AMD".to_string()),
        "10de" => Some("NVIDIA".to_string()),
        "8086" => Some("Intel".to_string()),
        _ => Some(format!("Unknown (0x{})", id)),
    }
}
