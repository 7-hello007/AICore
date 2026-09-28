//! Hardware implementer test.

use aicore_contracts::traits::hardware_executor::{
    HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionStatus, IHardwareExecutor,
};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxHardwareExecutor as PlatformExecutor;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsHardwareExecutor as PlatformExecutor;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn executor_has_name() {
    let e = PlatformExecutor::new();
    let name = e.executor_name();
    assert!(
        name == "linux_hardware_executor" || name == "windows_hardware_executor",
        "unexpected: {}",
        name
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_unknown_device_not_supported() {
    let e = PlatformExecutor::new();
    let r = e
        .apply(&HardwareExecutionRequest {
            device_id: "drm:nonexistent".to_string(),
            hint: HardwareExecutionHint::AutoMode,
        })
        .unwrap();
    assert!(matches!(
        r.status,
        HardwareExecutionStatus::NotSupported { .. }
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_bad_id_not_supported() {
    let e = PlatformExecutor::new();
    let r = e
        .apply(&HardwareExecutionRequest {
            device_id: "not-a-drm-id".to_string(),
            hint: HardwareExecutionHint::AutoMode,
        })
        .unwrap();
    assert!(matches!(
        r.status,
        HardwareExecutionStatus::NotSupported { .. }
    ));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_executor_always_not_supported() {
    let e = PlatformExecutor::new();
    let r = e
        .apply(&HardwareExecutionRequest {
            device_id: "drm:card0".to_string(),
            hint: HardwareExecutionHint::AutoMode,
        })
        .unwrap();
    assert!(matches!(
        r.status,
        HardwareExecutionStatus::NotSupported { .. }
    ));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn release_does_not_panic() {
    let e = PlatformExecutor::new();
    let _ = e.release("drm:card0");
}
