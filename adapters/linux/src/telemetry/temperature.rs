use aicore_contracts::telemetry::temperature::TemperatureSnapshot;

use super::sysfs;

const THERMAL_BASE: &str = "/sys/class/thermal";

/// Read system temperature.
///
/// `/sys/class/thermal/thermal_zone*`, by type:
/// - CPU: Type includes "cpu" or "x86_pkg_temp" or "k10temp" or "coretemp"
/// - GPU: Type includes "gpu" or "amdgpu" or "i915"
/// - Systems: Type includes "acpitz" or "pch"
///
/// Temperature units: sysfs are milligrade degrees centigrade (mC), converted to degrees Celsius.
///
/// Unable to read any value without panic, return all None Snapshot.
pub fn read_temperature() -> TemperatureSnapshot {
    let mut snapshot = TemperatureSnapshot::default();

    let zones = sysfs::list_dir(THERMAL_BASE);
    for zone in zones {
        let zone_path = format!("{}/{}", THERMAL_BASE, zone);
        if !zone.starts_with("thermal_zone") {
            continue;
        }

        let type_name = match sysfs::read_trimmed(format!("{}/type", zone_path)) {
            Some(t) => t.to_lowercase(),
            None => continue,
        };

        let temp_milli = match sysfs::read_i64(format!("{}/temp", zone_path)) {
            Some(t) => t,
            None => continue,
        };

        let celsius = temp_milli as f32 / 1000.0;

        if is_cpu_type(&type_name) && snapshot.cpu_celsius.is_none() {
            snapshot.cpu_celsius = Some(celsius);
        } else if is_gpu_type(&type_name) && snapshot.gpu_celsius.is_none() {
            snapshot.gpu_celsius = Some(celsius);
        } else if is_system_type(&type_name) && snapshot.system_celsius.is_none() {
            snapshot.system_celsius = Some(celsius);
        }
    }

    snapshot
}

fn is_cpu_type(name: &str) -> bool {
    name.contains("cpu")
        || name.contains("x86_pkg_temp")
        || name.contains("k10temp")
        || name.contains("coretemp")
        || name.contains("soc_thermal")
}

fn is_gpu_type(name: &str) -> bool {
    name.contains("gpu") || name.contains("amdgpu") || name.contains("i915")
}

fn is_system_type(name: &str) -> bool {
    name.contains("acpitz") || name.contains("pch") || name.contains("board")
}
