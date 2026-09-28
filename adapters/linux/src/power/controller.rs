use std::fs;
use std::io::ErrorKind;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::resource_policy::PowerMode,
    traits::power_controller::IPowerController,
};

/// Linux power controller.
///
/// Prefer `/sys/firmware/acpi/platform_profile` (modern kernel).
/// Fall back to cpufreq `scaling_governor` if not available.
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

fn parse_profile(value: &str, platform_profile: bool) -> ContractResult<PowerMode> {
    match (platform_profile, value) {
        (_, "performance") => Ok(PowerMode::Performance),
        (true, "low-power") | (false, "powersave") => Ok(PowerMode::PowerSave),
        (true, "balanced") | (false, "schedutil") => Ok(PowerMode::Balanced),
        _ => Err(ContractError::CapabilityUnavailable(format!(
            "unrecognized power profile: {value}"
        ))),
    }
}

impl IPowerController for LinuxPowerController {
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
        // Path 1: platform_profile
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
                    // Fall back to cpufreq
                },
                Err(e) => {
                    return Err(ContractError::AdapterUnavailable(format!(
                        "write {}: {}",
                        platform_path, e
                    )));
                },
            }
        }

        // Path 2: cpufreq governor; all available CPU, not just cpu0.
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
            return parse_profile(content.trim(), true);
        }
        let governor_path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
        if let Ok(content) = fs::read_to_string(governor_path) {
            return parse_profile(content.trim(), false);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_profile_never_claims_balanced() {
        assert!(matches!(
            parse_profile("vendor-custom", true),
            Err(ContractError::CapabilityUnavailable(_))
        ));
        assert!(matches!(
            parse_profile("ondemand", false),
            Err(ContractError::CapabilityUnavailable(_))
        ));
        assert_eq!(
            parse_profile("balanced", true).unwrap(),
            PowerMode::Balanced
        );
        assert_eq!(
            parse_profile("schedutil", false).unwrap(),
            PowerMode::Balanced
        );
    }
}
