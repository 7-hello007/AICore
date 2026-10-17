use aicore_contracts::hardware::capability::{
    HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet, HardwareDeviceKind,
};

/// Windows hardware capability detection.
///
/// The common ABI reports only detected capabilities; vendor-specific capabilities come from a runtime or plugin.
pub struct WindowsHardwareCapabilityProvider;
impl WindowsHardwareCapabilityProvider {
    pub fn new() -> Self {
        Self
    }
    pub fn gpu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
        let provider = super::telemetry::WindowsHardwareTelemetryProvider::new();
        let mut set = HardwareCapabilitySet::new(device_id);
        if !provider
            .gpu_devices()
            .iter()
            .any(|device| device.id == device_id)
        {
            return set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "exact device identity absent from provider enumeration",
            ));
        }
        set =
            set.with_device_kind(HardwareDeviceKind::Gpu)
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::DeviceIdentity,
                ));
        // Detection is separate from a successful telemetry observation.
        if let Ok(sample) = provider.sample_gpu(device_id) {
            if sample.device.id == device_id {
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
        }
        set.with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::VendorControl,
            "command presence does not establish reversible independently verified control",
        ))
    }
    pub fn npu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
        let set = HardwareCapabilitySet::new(device_id);
        if super::telemetry::WindowsHardwareTelemetryProvider::new()
            .npu_devices()
            .iter()
            .any(|d| d.id == device_id)
        {
            set.with_device_kind(HardwareDeviceKind::Npu)
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::DeviceIdentity,
                ))
        } else {
            set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "NPU identity unavailable; model execution requires a live provider",
            ))
        }
    }
}
impl Default for WindowsHardwareCapabilityProvider {
    fn default() -> Self {
        Self::new()
    }
}
