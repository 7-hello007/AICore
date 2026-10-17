use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::resource_policy::PowerMode,
    traits::power_controller::{IPowerController, PowerStateSnapshot},
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

// Snapshot entries contain only validated CPU indices and sysfs tokens, never paths.
#[derive(Clone)]
enum NativePowerState {
    Profile(String),
    Governors(Vec<(u32, String)>),
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn io_error(path: &Path, error: std::io::Error) -> ContractError {
    let message = format!("{}: {}", path.display(), error);
    match error.kind() {
        ErrorKind::PermissionDenied => ContractError::PermissionDenied(message),
        _ => ContractError::AdapterUnavailable(message),
    }
}

fn read_token(path: &Path) -> ContractResult<String> {
    let value = fs::read_to_string(path).map_err(|e| io_error(path, e))?;
    let value = value.trim();
    if !valid_token(value) {
        return Err(ContractError::ContractInvalid(
            "invalid sysfs power token".into(),
        ));
    }
    Ok(value.into())
}

fn governor_path(root: &Path, cpu: u32) -> std::path::PathBuf {
    root.join(format!(
        "devices/system/cpu/cpu{cpu}/cpufreq/scaling_governor"
    ))
}

fn write_verified(path: &Path, value: &str) -> ContractResult<()> {
    use std::io::Write;
    let metadata = fs::metadata(path).map_err(|e| io_error(path, e))?;
    if metadata.permissions().readonly() {
        return Err(ContractError::PermissionDenied(format!(
            "{} is read-only",
            path.display()
        )));
    }
    // Never create a vanished interface (including during restoration).
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| io_error(path, e))?;
    file.write_all(value.as_bytes())
        .map_err(|e| io_error(path, e))?;
    drop(file);
    if read_token(path)? != value {
        return Err(ContractError::AdapterUnavailable(format!(
            "{} power readback differs",
            path.display()
        )));
    }
    Ok(())
}

impl NativePowerState {
    fn read(root: &Path) -> ContractResult<Self> {
        let platform = root.join("firmware/acpi/platform_profile");
        match fs::metadata(&platform) {
            Ok(_) => return Ok(Self::Profile(read_token(&platform)?)),
            Err(e) if e.kind() == ErrorKind::NotFound => {},
            Err(e) => return Err(io_error(&platform, e)),
        }
        let cpu_root = root.join("devices/system/cpu");
        let entries = match fs::read_dir(&cpu_root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(ContractError::NotSupported(
                    "no power interface available".into(),
                ))
            },
            Err(e) => return Err(io_error(&cpu_root, e)),
        };
        let mut governors = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_error(&cpu_root, e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let cpu = match name.strip_prefix("cpu").and_then(|v| v.parse::<u32>().ok()) {
                Some(cpu) if name == format!("cpu{cpu}") => cpu,
                _ => continue,
            };
            let path = governor_path(root, cpu);
            match fs::metadata(&path) {
                Ok(_) => governors.push((cpu, read_token(&path)?)),
                Err(e) if e.kind() == ErrorKind::NotFound => {},
                Err(e) => return Err(io_error(&path, e)),
            }
        }
        governors.sort_by_key(|(cpu, _)| *cpu);
        if governors.is_empty() {
            return Err(ContractError::NotSupported(
                "no power interface available".into(),
            ));
        }
        Ok(Self::Governors(governors))
    }

    fn encode(&self) -> String {
        match self {
            Self::Profile(value) => format!("profile-v1\n{value}"),
            Self::Governors(values) => {
                let mut state = String::from("governors-v1");
                for (cpu, value) in values {
                    state.push_str(&format!("\ncpu{cpu}\t{value}"));
                }
                state
            },
        }
    }

    fn decode(state: &PowerStateSnapshot) -> ContractResult<Self> {
        let invalid = || ContractError::ContractInvalid("invalid Linux power snapshot".into());
        if state.platform != "linux" || state.state_id.len() > 1_048_576 {
            return Err(invalid());
        }
        let mut lines = state.state_id.split('\n');
        match lines.next() {
            Some("profile-v1") => {
                let value = lines
                    .next()
                    .filter(|value| valid_token(value))
                    .ok_or_else(invalid)?;
                if lines.next().is_some() {
                    return Err(invalid());
                }
                Ok(Self::Profile(value.into()))
            },
            Some("governors-v1") => {
                let mut values = Vec::new();
                let mut seen = std::collections::BTreeSet::new();
                for line in lines {
                    let (name, value) = line.split_once('\t').ok_or_else(invalid)?;
                    let cpu = name
                        .strip_prefix("cpu")
                        .and_then(|v| v.parse::<u32>().ok())
                        .ok_or_else(invalid)?;
                    if name != format!("cpu{cpu}") || !valid_token(value) || !seen.insert(cpu) {
                        return Err(invalid());
                    }
                    values.push((cpu, value.into()));
                }
                if values.is_empty() {
                    return Err(invalid());
                }
                Ok(Self::Governors(values))
            },
            _ => Err(invalid()),
        }
    }

    fn restore(&self, root: &Path) -> ContractResult<()> {
        let mut errors = Vec::new();
        match self {
            Self::Profile(value) => {
                if let Err(e) = write_verified(&root.join("firmware/acpi/platform_profile"), value)
                {
                    errors.push(e.to_string());
                }
            },
            Self::Governors(values) => {
                for (cpu, value) in values {
                    if let Err(e) = write_verified(&governor_path(root, *cpu), value) {
                        errors.push(e.to_string());
                    }
                }
            },
        }
        // A later write can affect an earlier CPU through a shared cpufreq
        // policy. Verify the complete final state after attempting every entry.
        let expected = match self {
            Self::Profile(value) => vec![(root.join("firmware/acpi/platform_profile"), value)],
            Self::Governors(values) => values
                .iter()
                .map(|(cpu, value)| (governor_path(root, *cpu), value))
                .collect(),
        };
        for (path, value) in expected {
            match read_token(&path) {
                Ok(actual) if actual == *value => {},
                Ok(_) => errors.push(format!(
                    "{} final restoration readback differs",
                    path.display()
                )),
                Err(error) => errors.push(error.to_string()),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ContractError::AdapterUnavailable(format!(
                "power restoration incomplete: {}",
                errors.join("; ")
            )))
        }
    }
}

impl LinuxPowerController {
    pub(crate) fn set_profile_at(root: &Path, mode: PowerMode) -> ContractResult<PowerMode> {
        let before = NativePowerState::read(root)?;
        let (profile, governor) = match mode {
            PowerMode::PowerSave => ("low-power", "powersave"),
            PowerMode::Balanced => ("balanced", "schedutil"),
            PowerMode::Performance => ("performance", "performance"),
        };
        let result = match &before {
            NativePowerState::Profile(_) => {
                write_verified(&root.join("firmware/acpi/platform_profile"), profile)
            },
            NativePowerState::Governors(values) => values
                .iter()
                .try_for_each(|(cpu, _)| write_verified(&governor_path(root, *cpu), governor)),
        };
        if let Err(error) = result {
            return match before.restore(root) {
                Ok(()) => Err(error),
                Err(recovery) => Err(ContractError::AdapterUnavailable(format!(
                    "{error}; {recovery}"
                ))),
            };
        }
        Ok(mode)
    }

    pub(crate) fn get_profile_at(root: &Path) -> ContractResult<PowerMode> {
        match NativePowerState::read(root)? {
            NativePowerState::Profile(value) => parse_profile(&value, true),
            NativePowerState::Governors(values) => {
                let first = &values[0].1;
                if values.iter().any(|(_, value)| value != first) {
                    return Err(ContractError::CapabilityUnavailable(
                        "CPU governors are mixed".into(),
                    ));
                }
                parse_profile(first, false)
            },
        }
    }

    pub(crate) fn capture_state_at(root: &Path) -> ContractResult<PowerStateSnapshot> {
        Ok(PowerStateSnapshot {
            platform: "linux".into(),
            state_id: NativePowerState::read(root)?.encode(),
        })
    }

    pub(crate) fn restore_state_at(root: &Path, state: &PowerStateSnapshot) -> ContractResult<()> {
        NativePowerState::decode(state)?.restore(root)
    }
}

impl IPowerController for LinuxPowerController {
    fn supports_exact_state(&self) -> bool {
        true
    }
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
        Self::set_profile_at(Path::new("/sys"), mode)
    }
    fn get_profile(&self) -> ContractResult<PowerMode> {
        Self::get_profile_at(Path::new("/sys"))
    }
    fn capture_state(&self) -> ContractResult<PowerStateSnapshot> {
        Self::capture_state_at(Path::new("/sys"))
    }
    fn restore_state(&self, state: &PowerStateSnapshot) -> ContractResult<()> {
        Self::restore_state_at(Path::new("/sys"), state)
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
