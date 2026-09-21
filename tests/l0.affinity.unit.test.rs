//! Windows affinity 能力声明测试。
//!
//! 在 Linux 上只能编译 + 检查能力声明；真实 Windows 调用留 CI。

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
    // 999_999_999 大概率不存在；无论成功/失败都不 panic
    let _ = c.get_affinity(999_999_999);
}
