use std::path::Path;

use aicore_contracts::telemetry::{battery::BatterySnapshot, power::PowerSnapshot};

use super::sysfs;

pub fn read_power() -> (BatterySnapshot, PowerSnapshot) {
    let (battery, mut power) = read_power_supply_at(Path::new("/sys/class/power_supply"));
    power.power_plan = sysfs::read_trimmed("/sys/firmware/acpi/platform_profile");
    (battery, power)
}

fn read_power_supply_at(root: &Path) -> (BatterySnapshot, PowerSnapshot) {
    let mut battery = BatterySnapshot::default();
    let mut power = PowerSnapshot::default();

    for entry in sysfs::list_dir(root) {
        let path = root.join(entry);
        match sysfs::read_trimmed(path.join("type")).as_deref() {
            Some("Battery") => {
                if sysfs::read_trimmed(path.join("present")).as_deref() == Some("0") {
                    continue;
                }
                battery.present = true;
                battery.percentage = sysfs::read_f32(path.join("capacity"))
                    .filter(|value| value.is_finite() && (0.0..=100.0).contains(value));
                battery.charging = match sysfs::read_trimmed(path.join("status")).as_deref() {
                    Some("Charging") => Some(true),
                    Some("Discharging" | "Full" | "Not charging") => Some(false),
                    _ => None,
                };
                if battery.charging == Some(true) {
                    power.ac_connected = true;
                }
            },
            Some("Mains" | "USB") => {
                if sysfs::read_trimmed(path.join("online")).as_deref() == Some("1") {
                    power.ac_connected = true;
                }
            },
            _ => {},
        }
    }

    (battery, power)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_and_ac_are_read_from_isolated_fixture() {
        let root = std::env::temp_dir().join(format!(
            "aicore-power-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let battery = root.join("BAT0");
        let mains = root.join("AC0");
        std::fs::create_dir_all(&battery).unwrap();
        std::fs::create_dir_all(&mains).unwrap();
        std::fs::write(battery.join("type"), "Battery").unwrap();
        std::fs::write(battery.join("capacity"), "74").unwrap();
        std::fs::write(battery.join("status"), "Charging").unwrap();
        std::fs::write(mains.join("type"), "Mains").unwrap();
        std::fs::write(mains.join("online"), "1").unwrap();

        let (battery, power) = read_power_supply_at(&root);
        assert!(battery.present);
        assert_eq!(battery.percentage, Some(74.0));
        assert_eq!(battery.charging, Some(true));
        assert!(power.ac_connected);
        std::fs::remove_dir_all(root).unwrap();
    }
}
