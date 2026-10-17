use aicore_contracts::telemetry::npu::NpuSnapshot;

use super::sysfs;

/// NPU detection.
///
/// There is no single NPU interface on Linux. Several known paths are detected:
/// - Intel VPU: `/sys/class/accel/accel*`
/// - AMD XDNA: No stable sysfs interface
/// - Qualcomm: no stable sysfs interface
///
/// Detection supplies identity only. `available` requires a valid telemetry sample;
/// it does not advertise inference or control support.
pub fn read_npu() -> NpuSnapshot {
    read_npu_from(std::path::Path::new("/sys/class/accel"))
}

fn read_npu_from(base: &std::path::Path) -> NpuSnapshot {
    let mut snapshot = NpuSnapshot::default();

    // Intel VPU/ Generic accel device
    let accel = sysfs::list_dir(base);
    for entry in accel {
        if !entry
            .strip_prefix("accel")
            .map(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or(false)
        {
            continue;
        }
        snapshot.usage_percent = sysfs::read_f32(base.join(&entry).join("device/utilization"))
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value));
        snapshot.available = snapshot.usage_percent.is_some();
        snapshot.name = Some(entry.clone());
        snapshot.vendor =
            match sysfs::read_trimmed(base.join(&entry).join("device/vendor")).as_deref() {
                Some("0x8086") => Some("Intel".into()),
                Some("0x1002") => Some("AMD".into()),
                Some("0x10de") => Some("NVIDIA".into()),
                _ => None,
            };
        break;
    }

    snapshot
}

#[cfg(test)]
mod p0_tests {
    use super::*;
    #[test]
    fn p0_generic_accel_detection_does_not_claim_intel_or_available_telemetry() {
        let root = std::env::temp_dir().join(format!(
            "aicore-npu-p0-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("accel0")).unwrap();
        let observed = read_npu_from(&root);
        assert!(root
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(observed.name.as_deref(), Some("accel0"));
        assert!(
            !observed.available,
            "detected interface alone is not available telemetry"
        );
        assert_eq!(
            observed.vendor, None,
            "unknown accel vendor must not be invented"
        );
    }
}
