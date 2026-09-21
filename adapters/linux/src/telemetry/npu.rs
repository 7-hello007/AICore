use aicore_contracts::telemetry::npu::NpuSnapshot;

use super::sysfs;

/// NPU 探测。
///
/// Linux 上没有统一 NPU 接口。检测几个已知路径：
/// - Intel VPU: `/sys/class/accel/accel*`
/// - AMD XDNA: 无稳定 sysfs 接口
/// - Qualcomm: 无稳定 sysfs 接口
///
/// 找不到时返回 `available: false`。
pub fn read_npu() -> NpuSnapshot {
    let mut snapshot = NpuSnapshot::default();

    // Intel VPU / 通用 accel 设备
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
