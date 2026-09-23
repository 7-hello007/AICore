use aicore_contracts::{
    policy::{
        priority::PolicyPriority,
        resource_policy::{PowerMode, ResourcePolicy},
    },
    telemetry::snapshot::ComputerSnapshot,
};

/// Conservative, telemetry-gated policy constraint. Missing or non-finite
/// readings never trigger a claim of thermal/power optimization.
pub struct ThermalPowerGuard {
    hot_celsius: f32,
    low_battery_percent: f32,
}

impl ThermalPowerGuard {
    pub fn new(hot_celsius: f32, low_battery_percent: f32) -> Self {
        Self {
            hot_celsius: if hot_celsius.is_finite() {
                hot_celsius.clamp(50.0, 120.0)
            } else {
                85.0
            },
            low_battery_percent: if low_battery_percent.is_finite() {
                low_battery_percent.clamp(0.0, 100.0)
            } else {
                20.0
            },
        }
    }

    pub fn constrain(&self, snapshot: &ComputerSnapshot, policy: &mut ResourcePolicy) -> bool {
        let hot = [
            snapshot.temperature.cpu_celsius,
            snapshot.temperature.gpu_celsius,
            snapshot.temperature.system_celsius,
        ]
        .into_iter()
        .flatten()
        .any(|temperature| temperature.is_finite() && temperature >= self.hot_celsius);
        let low_battery = snapshot.battery.present
            && snapshot.battery.charging == Some(false)
            && snapshot.battery.percentage.is_some_and(|percentage| {
                percentage.is_finite() && percentage <= self.low_battery_percent
            });
        if !hot && !low_battery {
            return false;
        }
        policy.power.mode = PowerMode::PowerSave;
        if hot {
            policy.gpu.priority = PolicyPriority::Low;
            policy.npu.priority = PolicyPriority::Low;
        }
        policy.reason.push_str(if hot {
            "; thermal guard"
        } else {
            "; low-battery guard"
        });
        true
    }
}

impl Default for ThermalPowerGuard {
    fn default() -> Self {
        Self::new(85.0, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_mock_snapshot_constrains_only_existing_policy() {
        let mut snapshot = ComputerSnapshot::default();
        snapshot.temperature.gpu_celsius = Some(91.0);
        let mut policy = ResourcePolicy::default();
        assert!(ThermalPowerGuard::default().constrain(&snapshot, &mut policy));
        assert_eq!(policy.power.mode, PowerMode::PowerSave);
        assert_eq!(policy.gpu.priority, PolicyPriority::Low);
    }

    #[test]
    fn unavailable_or_nan_reading_does_not_fake_optimization() {
        let mut snapshot = ComputerSnapshot::default();
        snapshot.temperature.cpu_celsius = Some(f32::NAN);
        let mut policy = ResourcePolicy::default();
        assert!(!ThermalPowerGuard::default().constrain(&snapshot, &mut policy));
        assert_eq!(policy.power.mode, PowerMode::Balanced);
    }

    #[test]
    fn non_finite_configuration_uses_safe_thresholds() {
        let mut snapshot = ComputerSnapshot::default();
        snapshot.temperature.cpu_celsius = Some(90.0);
        let mut policy = ResourcePolicy::default();
        assert!(ThermalPowerGuard::new(f32::NAN, f32::INFINITY).constrain(&snapshot, &mut policy));
        assert_eq!(policy.power.mode, PowerMode::PowerSave);
    }
}
