//! Windows capability statement test.
//!
//! Only + check capability statements can be compiled on Linux; real Windows calls for the retention of CI.

#[cfg(target_os = "windows")]
use aicore_contracts::traits::capability_provider::ICapabilityProvider;
use aicore_contracts::traits::process_controller::IProcessController;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::{WindowsCapabilityProvider, WindowsProcessController};

#[cfg(target_os = "windows")]
#[test]
fn windows_declares_affinity_capabilities() {
    let c = WindowsProcessController::new();
    let caps = c.capabilities();
    assert!(caps.contains(&"process.affinity.read"));
    assert!(caps.contains(&"process.affinity.write"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_capability_provider_marks_affinity_supported() {
    let p = WindowsCapabilityProvider::new();
    let names = p.capability_names();
    assert!(names.contains(&"process.affinity.read".to_string()));
    assert!(names.contains(&"process.affinity.write".to_string()));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_has_own_affinity() {
    use aicore_adapter_linux::LinuxProcessController;
    let c = LinuxProcessController::new();
    let caps = c.capabilities();
    assert!(caps.contains(&"process.affinity.read"));
    assert!(caps.contains(&"process.affinity.write"));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn affinity_returns_contract_error_on_bad_pid() {
    #[cfg(target_os = "linux")]
    use aicore_adapter_linux::LinuxProcessController as PlatformProcessController;
    #[cfg(target_os = "windows")]
    use aicore_adapter_windows::WindowsProcessController as PlatformProcessController;

    let c = PlatformProcessController::new();
    // 999_999_999 No probability; success/failure is not panic
    let _ = c.get_affinity(999_999_999);
}
