use std::path::Path;

use aicore_contracts::hardware::capability::{
    HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet, HardwareDeviceKind,
};

/// Linux hardware capability detection.
pub struct LinuxHardwareCapabilityProvider;

impl LinuxHardwareCapabilityProvider {
    pub fn new() -> Self {
        Self
    }

    /// Detection of GPU capability.
    pub fn gpu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        let Some(device_path) = super::telemetry::gpu_device_path(device_id) else {
            return HardwareCapabilitySet::new(device_id).with(
                HardwareCapabilityEntry::unsupported(
                    HardwareCapability::DeviceIdentity,
                    "exact device absent from OS discovery",
                ),
            );
        };
        let device_path = device_path.to_string_lossy();
        let mut set = HardwareCapabilitySet::new(device_id);

        if !Path::new(device_path.as_ref()).exists() {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "device path not found",
            ));
            return set;
        }

        set =
            set.with_device_kind(HardwareDeviceKind::Gpu)
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::DeviceIdentity,
                ));

        use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
        if let Ok(sample) =
            super::telemetry::LinuxHardwareTelemetryProvider::new().sample_gpu(device_id)
        {
            for (capability, available) in [
                (
                    HardwareCapability::UtilizationTelemetry,
                    sample.utilization_percent.is_some(),
                ),
                (
                    HardwareCapability::MemoryTelemetry,
                    sample.memory_total_mb.is_some(),
                ),
                (
                    HardwareCapability::TemperatureTelemetry,
                    sample.temperature_celsius.is_some(),
                ),
            ] {
                if available {
                    set = set.with(HardwareCapabilityEntry::supported(capability));
                }
            }
        }

        set = set.with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::WorkloadHint,
            "sysfs path detection does not establish reversible guarded control",
        ));

        set
    }

    /// Detection of NPU capability.
    pub fn npu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        let Some(path) = super::telemetry::npu_device_path(device_id) else {
            return HardwareCapabilitySet::new(device_id).with(
                HardwareCapabilityEntry::unsupported(
                    HardwareCapability::DeviceIdentity,
                    "exact device absent from OS discovery",
                ),
            );
        };
        let mut set = HardwareCapabilitySet::new(device_id);

        if !path.exists() {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "accel device not found",
            ));
            return set;
        }

        set =
            set.with_device_kind(HardwareDeviceKind::Npu)
                .with(HardwareCapabilityEntry::supported(
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
