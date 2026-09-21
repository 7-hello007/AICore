use std::path::Path;

use aicore_contracts::{
    capability::entry::CapabilityEntry, traits::capability_provider::ICapabilityProvider,
};

/// Linux 能力提供者。
///
/// 运行时探测：
/// - `/sys/firmware/acpi/platform_profile` → platform_profile 支持。
/// - `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` → cpufreq 支持。
///
/// 未实现在 V2-A-2 范围内的能力（GPU / NPU / affinity）
/// 显式标记为 Unsupported，附 detail 说明。
pub struct LinuxCapabilityProvider;

impl LinuxCapabilityProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxCapabilityProvider {
    fn default() -> Self {
        Self::new()
    }
}

const PLATFORM_PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
const CPUFREQ_GOVERNOR_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";

impl ICapabilityProvider for LinuxCapabilityProvider {
    fn capabilities_detailed(&self) -> Vec<CapabilityEntry> {
        // 无条件支持的能力
        let mut caps = vec![
            CapabilityEntry::supported("cpu.telemetry"),
            CapabilityEntry::supported("memory.telemetry"),
            CapabilityEntry::supported("disk.telemetry"),
            CapabilityEntry::supported("network.telemetry"),
            CapabilityEntry::supported("process.telemetry"),
            CapabilityEntry::supported("process.priority.read"),
            CapabilityEntry::supported("process.priority.write"),
        ];

        // 电源：运行时探测
        let has_platform_profile = Path::new(PLATFORM_PROFILE_PATH).exists();
        let has_cpufreq = Path::new(CPUFREQ_GOVERNOR_PATH).exists();

        if has_platform_profile {
            caps.push(CapabilityEntry::supported("power.profile.platform_profile"));
        }
        if has_cpufreq {
            caps.push(CapabilityEntry::supported("power.profile.cpufreq"));
        }
        if !has_platform_profile && !has_cpufreq {
            caps.push(CapabilityEntry::unsupported(
                "power.profile",
                "no platform_profile or cpufreq governor available",
            ));
        }

        // 温度：Linux 上 /sys/class/thermal 普遍存在
        let has_thermal = Path::new("/sys/class/thermal").exists();
        if has_thermal {
            caps.push(CapabilityEntry::supported("temperature.telemetry"));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "temperature.telemetry",
                "/sys/class/thermal not available",
            ));
        }

        // GPU 遥测：尽力而为（amdgpu / i915 sysfs）
        let has_drm = Path::new("/sys/class/drm").exists();
        if has_drm {
            caps.push(CapabilityEntry::partial(
                "gpu.telemetry",
                "best effort via /sys/class/drm; vendor coverage varies",
            ));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "gpu.telemetry",
                "/sys/class/drm not available",
            ));
        }
        let amd_gpu_control = std::fs::read_dir("/sys/class/drm")
            .ok()
            .map(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with("card")
                        && !name.contains('-')
                        && e.path()
                            .join("device/power_dpm_force_performance_level")
                            .exists()
                })
            })
            .unwrap_or(false);
        if amd_gpu_control {
            caps.push(CapabilityEntry::partial(
                "gpu.control",
                "AMD amdgpu performance-level control available",
            ));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "gpu.control",
                "no generic GPU control path detected",
            ));
        }

        // NPU 遥测：检测 /sys/class/accel
        let has_accel = Path::new("/sys/class/accel").exists();
        if has_accel {
            caps.push(CapabilityEntry::partial(
                "npu.telemetry",
                "best effort via /sys/class/accel",
            ));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "npu.telemetry",
                "no /sys/class/accel device",
            ));
        }
        caps.push(CapabilityEntry::unsupported(
            "npu.control",
            "vendor runtime required; Linux has no portable cross-vendor NPU control ABI",
        ));

        // 前台窗口：xdotool 可用时 partial
        let has_xdotool = std::process::Command::new("xdotool")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if has_xdotool {
            caps.push(CapabilityEntry::partial(
                "foreground.telemetry",
                "via xdotool; only works on X11",
            ));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "foreground.telemetry",
                "xdotool not installed or Wayland session",
            ));
        }

        // CPU affinity：当前实现 后，需要权限
        caps.push(CapabilityEntry::supported("process.affinity.read"));
        caps.push(CapabilityEntry::supported("process.affinity.write"));

        caps
    }

    fn component_name(&self) -> &'static str {
        "linux_capability"
    }
}
