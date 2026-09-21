use std::process::Command;

use aicore_contracts::hardware::capability::{
    HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet,
};

/// Windows 硬件能力探测。
///
/// 通用 ABI 不存在时只报告已检测到的能力；vendor-specific capability 由运行时/插件提供。
pub struct WindowsHardwareCapabilityProvider;
impl WindowsHardwareCapabilityProvider {
    pub fn new() -> Self {
        Self
    }
    fn nvidia_available() -> bool {
        Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    pub fn gpu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        let mut set = HardwareCapabilitySet::new(device_id);
        if device_id.starts_with("nvidia:") && Self::nvidia_available() {
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::DeviceIdentity,
            ));
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::UtilizationTelemetry,
            ));
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::MemoryTelemetry,
            ));
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::TemperatureTelemetry,
            ));
            set = set.with(HardwareCapabilityEntry::supported(
                HardwareCapability::VendorControl,
            ));
        } else {
            set = set.with(HardwareCapabilityEntry::unsupported(
                HardwareCapability::DeviceIdentity,
                "no supported vendor hardware provider detected",
            ));
        }
        set
    }
    pub fn npu_capability(&self, device_id: &str) -> HardwareCapabilitySet {
        HardwareCapabilitySet::new(device_id).with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::DeviceIdentity,
            "Windows NPU requires Windows ML/vendor runtime adapter",
        ))
    }
}
impl Default for WindowsHardwareCapabilityProvider {
    fn default() -> Self {
        Self::new()
    }
}
