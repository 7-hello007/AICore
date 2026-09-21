use std::fs;
use std::io::ErrorKind;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::resource_policy::PowerMode,
    traits::power_controller::IPowerController,
};

/// Linux 电源控制器。
///
/// 优先使用 `/sys/firmware/acpi/platform_profile`（现代内核）。
/// 若不可用，回退到 cpufreq `scaling_governor`（按 cpu0 代表）。
pub struct LinuxPowerController;

impl LinuxPowerController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxPowerController {
    fn default() -> Self {
        Self::new()
    }
}

impl IPowerController for LinuxPowerController {
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
        // 路径 1：platform_profile
        let platform_path = "/sys/firmware/acpi/platform_profile";
        if std::path::Path::new(platform_path).exists() {
            let value = match mode {
                PowerMode::PowerSave => "low-power",
                PowerMode::Balanced => "balanced",
                PowerMode::Performance => "performance",
            };
            match fs::write(platform_path, value) {
                Ok(()) => return Ok(mode),
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    return Err(ContractError::PermissionDenied(format!(
                        "write {}: {}",
                        platform_path, e
                    )));
                },
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // 落到 cpufreq 回退
                },
                Err(e) => {
                    return Err(ContractError::AdapterUnavailable(format!(
                        "write {}: {}",
                        platform_path, e
                    )));
                },
            }
        }

        // 路径 2：cpufreq governor；遍历所有可用 CPU，而非只写 cpu0。
        let value = match mode {
            PowerMode::PowerSave => "powersave",
            PowerMode::Balanced => "schedutil",
            PowerMode::Performance => "performance",
        };
        let mut found = false;
        let cpu_root = std::path::Path::new("/sys/devices/system/cpu");
        if let Ok(entries) = fs::read_dir(cpu_root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("cpu") || !name[3..].chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let governor_path = entry.path().join("cpufreq").join("scaling_governor");
                if !governor_path.exists() {
                    continue;
                }
                found = true;
                if let Err(e) = fs::write(&governor_path, value) {
                    if e.kind() == ErrorKind::PermissionDenied {
                        return Err(ContractError::PermissionDenied(format!(
                            "write {}: {}",
                            governor_path.display(),
                            e
                        )));
                    }
                    return Err(ContractError::AdapterUnavailable(format!(
                        "write {}: {}",
                        governor_path.display(),
                        e
                    )));
                }
            }
        }
        if found {
            return Ok(mode);
        }

        Err(ContractError::NotSupported(
            "no platform_profile or cpufreq governor available".to_string(),
        ))
    }

    fn get_profile(&self) -> ContractResult<PowerMode> {
        let platform_path = "/sys/firmware/acpi/platform_profile";
        if let Ok(content) = fs::read_to_string(platform_path) {
            let v = content.trim();
            return Ok(match v {
                "low-power" => PowerMode::PowerSave,
                "performance" => PowerMode::Performance,
                _ => PowerMode::Balanced,
            });
        }
        let governor_path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
        if let Ok(content) = fs::read_to_string(governor_path) {
            let v = content.trim();
            return Ok(match v {
                "powersave" => PowerMode::PowerSave,
                "performance" => PowerMode::Performance,
                _ => PowerMode::Balanced,
            });
        }
        Err(ContractError::NotSupported(
            "no power interface available".to_string(),
        ))
    }

    fn capabilities(&self) -> Vec<&'static str> {
        let mut caps = Vec::new();
        if std::path::Path::new("/sys/firmware/acpi/platform_profile").exists() {
            caps.push("power.profile.platform_profile");
        }
        if std::path::Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").exists() {
            caps.push("power.profile.cpufreq");
        }
        caps
    }

    fn platform_name(&self) -> &'static str {
        "linux"
    }
}
