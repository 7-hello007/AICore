use aicore_contracts::telemetry::gpu::GpuSnapshot;

use super::sysfs;

pub(crate) const DRM_BASE: &str = "/sys/class/drm";

/// Read GPU telemetry.
///
/// Support path:
/// - AMD (amdgpu): `/sys/class/drm/card*/device/gpu_busy_percent`
/// VRAM: `/sys/class/drm/card*/device/mem_info_vram_used` and `_total`
/// - Temperature: `/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input`
///
/// If no GPU is found, or a field cannot be read, return `None` values without panicking.
pub fn read_gpu() -> GpuSnapshot {
    read_gpu_with_identity(std::path::Path::new(DRM_BASE)).0
}

pub(crate) fn read_gpu_with_identity(base: &std::path::Path) -> (GpuSnapshot, Option<String>) {
    let mut detected = GpuSnapshot::default();
    let mut detected_id = None;

    let cards = sysfs::list_dir(base);
    for card in cards {
        if !card.starts_with("card") || card.contains('-') {
            // Skip a connector like "card0-DP-1"
            continue;
        }

        let device_path = base
            .join(&card)
            .join("device")
            .to_string_lossy()
            .into_owned();
        if !std::path::Path::new(&device_path).exists() {
            continue;
        }

        // Missing fields remain missing for this device. Never fill them from
        // another card while retaining the first card's name/vendor.
        let mut snapshot = GpuSnapshot::default();

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
                snapshot.usage_percent =
                    Some(v).filter(|value| value.is_finite() && (0.0..=100.0).contains(value));
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
        if snapshot.usage_percent.is_some()
            || snapshot.memory_total_mb.is_some()
            || snapshot.memory_used_mb.is_some()
            || snapshot.temperature_celsius.is_some()
        {
            let identity = crate::hardware::telemetry::stable_device_id(
                "drm",
                std::path::Path::new(&device_path),
            );
            return (snapshot, identity);
        }
        if detected.name.is_none() {
            detected_id = crate::hardware::telemetry::stable_device_id(
                "drm",
                std::path::Path::new(&device_path),
            );
            detected = snapshot;
        }
    }

    (detected, detected_id)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn p1_gpu_fields_never_mix_different_devices() {
        let root = std::env::temp_dir().join(format!(
            "aicore-p1-gpus-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for (card, vendor, id) in [("card0", "0x1002", "0x1111"), ("card1", "0x10de", "0x2222")] {
            let device = root.join(card).join("device");
            std::fs::create_dir_all(&device).unwrap();
            std::fs::write(device.join("vendor"), vendor).unwrap();
            std::fs::write(device.join("device"), id).unwrap();
        }
        std::fs::write(root.join("card0/device/gpu_busy_percent"), "30").unwrap();
        std::fs::write(root.join("card1/device/mem_info_vram_total"), "134217728").unwrap();
        let (first, first_id) = read_gpu_with_identity(&root);
        std::fs::remove_file(root.join("card0/device/gpu_busy_percent")).unwrap();
        let (second, second_id) = read_gpu_with_identity(&root);
        assert_eq!(
            first_id,
            Some(format!(
                "drm:sysfs:{}",
                root.join("card0/device").canonicalize().unwrap().display()
            ))
        );
        assert_eq!(
            second_id,
            Some(format!(
                "drm:sysfs:{}",
                root.join("card1/device").canonicalize().unwrap().display()
            ))
        );
        assert_ne!(first_id, second_id);
        assert!(root
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(first.vendor.as_deref(), Some("AMD"));
        assert_eq!(first.usage_percent, Some(30.0));
        assert_eq!(
            first.memory_total_mb, None,
            "card1 memory cannot be attributed to card0"
        );
        assert_eq!(second.vendor.as_deref(), Some("NVIDIA"));
        assert_eq!(second.memory_total_mb, Some(128));
        assert!(second.name.unwrap().contains("0x2222"));
    }
}
