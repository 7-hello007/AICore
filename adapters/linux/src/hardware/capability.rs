use std::path::Path;

use aicore_contracts::hardware::capability::{
    HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet,
};

const DRM_BASE: &str = "/sys/class/drm";
const ACCEL_BASE: &str = "/sys/class/accel";

/// Linux 硬件能力探测。
pub struct LinuxHardwareCapabilityProvider;

impl LinuxHardwareCapabilityProvider {
    pub fn new() -> Self {
        Self
    }

    /// 探测 GPU 能力。
    pub fn gpu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        let card = match device_id.strip_prefix("drm:") {
            Some(c) => c,
            None => return HardwareCapabilitySet::new(device_id),
        };
        let device_path = format!("{}/{}/device", DRM_BASE, card);
        let mut set = HardwareCapabilitySet::new(device_id);

        if !Path::new(&device_path).exists() {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "device path not found",
            ));
            return set;
        }

        set = set.with(HardwareCapabilityEntry::supported(
            HardwareCapability::DeviceIdentity,
        ));

        if Path::new(&format!("{}/gpu_busy_percent", device_path)).exists() {
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::UtilizationTelemetry,
            ));
        }
        if Path::new(&format!("{}/mem_info_vram_total", device_path)).exists() {
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::MemoryTelemetry,
            ));
        }

        // 温度：hwmon 目录存在即视为支持
        let hwmon_path = format!("{}/hwmon", device_path);
        if Path::new(&hwmon_path).exists() {
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::TemperatureTelemetry,
            ));
        }

        // workload hint：amdgpu performance level
        if Path::new(&format!(
            "{}/power_dpm_force_performance_level",
            device_path
        ))
        .exists()
        {
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::WorkloadHint,
            ));
        } else {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::WorkloadHint,
                "no power_dpm_force_performance_level",
            ));
        }

        set
    }

    /// 探测 NPU 能力。
    pub fn npu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        let entry = match device_id.strip_prefix("npu:") {
            Some(e) => e,
            None => return HardwareCapabilitySet::new(device_id),
        };
        let path = format!("{}/{}", ACCEL_BASE, entry);
        let mut set = HardwareCapabilitySet::new(device_id);

        if !Path::new(&path).exists() {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "accel device not found",
            ));
            return set;
        }

        set = set.with(HardwareCapabilityEntry::supported(
            HardwareCapability::DeviceIdentity,
        ));
        set = set.with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::WorkloadHint,
            "Linux has no portable cross-vendor NPU control ABI; vendor runtime required",
        ));
        set = set.with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::VendorControl,
            "vendor SDK/runtime required for NPU control",
        ));
        set
    }
}

impl Default for LinuxHardwareCapabilityProvider {
    fn default() -> Self {
        Self::new()
    }
}
