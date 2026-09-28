//! Hardware capability detection test.

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxHardwareCapabilityProvider as PlatformProvider;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsHardwareCapabilityProvider as PlatformProvider;

use aicore_contracts::hardware::capability::HardwareCapability;

#[cfg(target_os = "linux")]
#[test]
fn linux_unknown_gpu_unsupported_identity() {
    let p = PlatformProvider::new();
    let set = p.gpu_capability("drm:nonexistent");
    assert!(!set.is_supported(HardwareCapability::DeviceIdentity));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_unknown_npu_unsupported_identity() {
    let p = PlatformProvider::new();
    let set = p.npu_capability("npu:nonexistent");
    assert!(!set.is_supported(HardwareCapability::DeviceIdentity));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_gpu_unsupported() {
    let p = PlatformProvider::new();
    let set = p.gpu_capability("drm:card0");
    assert!(!set.is_supported(HardwareCapability::DeviceIdentity));
}
