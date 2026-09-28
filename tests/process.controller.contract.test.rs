use aicore_contracts::traits::process_controller::IProcessController;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxProcessController as PlatformProcessController;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsProcessController as PlatformProcessController;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn controller_reports_platform() {
    let c = PlatformProcessController::new();
    let name = c.platform_name();
    assert!(name == "linux" || name == "windows");
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn controller_handles_bad_pid_gracefully() {
    use aicore_contracts::policy::priority::PolicyPriority;
    let c = PlatformProcessController::new();
    // 一个大概率不存在的 PID
    let result = c.set_priority(999_999_999, PolicyPriority::Low);
    assert!(result.is_err(), "invalid PID must not succeed");
}
