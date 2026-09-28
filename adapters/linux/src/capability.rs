use std::path::Path;

use aicore_contracts::{
    capability::entry::CapabilityEntry, traits::capability_provider::ICapabilityProvider,
};

/// Linux capability provider.
///
/// Runtime detection:
/// - `/sys/firmware/acpi/platform_profile` → `platform_profile` support.
/// - `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` → `cpufreq` support.
///
/// Capabilities not implemented in V2-A-2 (GPU/NPU/affinity)
/// are explicitly marked `Unsupported` with explanatory details.
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
        // Unconditionally supported capabilities
        let mut caps = vec![
            CapabilityEntry::supported("cpu.telemetry"),
            CapabilityEntry::supported("memory.telemetry"),
            CapabilityEntry::supported("disk.telemetry"),
            CapabilityEntry::supported("network.telemetry"),
            CapabilityEntry::supported("process.telemetry"),
            CapabilityEntry::supported("process.priority.read"),
            CapabilityEntry::conditional(
                "process.priority.write",
                "limited to processes permitted by the current UID and OS policy",
            ),
        ];

        // Power capabilities are detected at runtime.
        let has_platform_profile = Path::new(PLATFORM_PROFILE_PATH).exists();
        let has_cpufreq = Path::new(CPUFREQ_GOVERNOR_PATH).exists();

        if has_platform_profile {
            caps.push(CapabilityEntry::conditional(
                "power.profile.platform_profile",
                "interface detected; write permission and profile support vary",
            ));
        }
        if has_cpufreq {
            caps.push(CapabilityEntry::conditional(
                "power.profile.cpufreq",
                "interface detected; governor and write permission vary",
            ));
        }
        if !has_platform_profile && !has_cpufreq {
            caps.push(CapabilityEntry::unsupported(
                "power.profile",
                "no platform_profile or cpufreq governor available",
            ));
        }

        // Linux commonly exposes temperature data through /sys/class/thermal.
        let has_thermal = Path::new("/sys/class/thermal").exists();
        if has_thermal {
            caps.push(CapabilityEntry::supported("temperature.telemetry"));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "temperature.telemetry",
                "/sys/class/thermal not available",
            ));
        }

        // GPU telemetry: best effort (amdgpu / i915 sysfs)
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
            caps.push(CapabilityEntry::conditional(
                "gpu.control",
                "AMD amdgpu performance-level path detected; write permission unverified",
            ));
        } else {
            caps.push(CapabilityEntry::unsupported(
                "gpu.control",
                "no generic GPU control path detected",
            ));
        }

        // Detect NPU telemetry support through /sys/class/accel.
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

        // Foreground window: partial when xdotool is available
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

        // CPU-affinity writes require permission even when the interface is available.
        caps.push(CapabilityEntry::supported("process.affinity.read"));
        caps.push(CapabilityEntry::conditional(
            "process.affinity.write",
            "limited to processes permitted by the current UID, cpuset and OS policy",
        ));

        caps
    }

    fn component_name(&self) -> &'static str {
        "linux_capability"
    }
}
