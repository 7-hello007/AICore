use aicore_contracts::telemetry::npu::NpuSnapshot;

use super::sysfs;

/// NPU detection.
///
/// There is no single NPU interface on Linux. Several known paths are detected:
/// - Intel VPU: `/sys/class/accel/accel*`
/// - AMD XDNA: No stable sysfs interface
/// - Qualcomm: no stable sysfs interface
///
/// Returns `available: false` when found.
pub fn read_npu() -> NpuSnapshot {
    let mut snapshot = NpuSnapshot::default();

    // Intel VPU/ Generic accel device
    let accel = sysfs::list_dir("/sys/class/accel");
    for entry in accel {
        if !entry.starts_with("accel") {
            continue;
        }
        snapshot.available = true;
        snapshot.name = Some(entry.clone());
        snapshot.vendor = Some("Intel".to_string());
        break;
    }

    snapshot
}
