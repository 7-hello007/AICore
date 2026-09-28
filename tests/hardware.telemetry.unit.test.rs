//! 硬件遥测测试。

use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxHardwareTelemetryProvider as PlatformProvider;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsHardwareTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn provider_has_name() {
    let p = PlatformProvider::new();
    let name = p.provider_name();
    assert!(
        name == "linux_hardware_telemetry" || name == "windows_hardware_telemetry",
        "unexpected: {}",
        name
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_enumerates_gpu_devices_without_panic() {
    let p = PlatformProvider::new();
    let devices = p.gpu_devices();
    // 容器 / VM 可能没有 GPU；只验证不 panic
    for d in &devices {
        assert!(d.id.starts_with("drm:"));
        assert!(!d.vendor.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_sample_gpu_returns_result_or_error() {
    let p = PlatformProvider::new();
    let devices = p.gpu_devices();
    if let Some(d) = devices.first() {
        let _ = p.sample_gpu(&d.id);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_sample_unknown_gpu_fails() {
    let p = PlatformProvider::new();
    let err = p.sample_gpu("drm:nonexistent").unwrap_err();
    assert!(format!("{}", err).contains("not found"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_gpu_telemetry_rejects_unknown_id_without_hardware_assumptions() {
    let p = PlatformProvider::new();
    let devices = p.gpu_devices();
    for device in &devices {
        assert!(device.id.starts_with("nvidia:") || device.id.starts_with("gpu:dxgi:"));
        assert!(!device.name.trim().is_empty() && !device.vendor.trim().is_empty());
        assert!(
            aicore_adapter_windows::WindowsHardwareCapabilityProvider::new()
                .gpu_capability(&device.id)
                .is_supported(
                    aicore_contracts::hardware::capability::HardwareCapability::DeviceIdentity
                )
        );
    }
    assert!(matches!(
        p.sample_gpu("gpu:dxgi:invalid"),
        Err(aicore_contracts::errors::ContractError::InvalidRequest(_))
    ));
    let err = p.sample_gpu("drm:card0").unwrap_err();
    assert!(matches!(
        err,
        aicore_contracts::errors::ContractError::InvalidRequest(_)
    ));
    assert!(matches!(
        p.sample_npu("npu:unknown"),
        Err(aicore_contracts::errors::ContractError::NotSupported(_))
    ));
}
