use aicore_contracts::traits::power_controller::IPowerController;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxPowerController as PlatformPowerController;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsPowerController as PlatformPowerController;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn controller_reports_platform() {
    let c = PlatformPowerController::new();
    let name = c.platform_name();
    assert!(name == "linux" || name == "windows");
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn get_profile_does_not_panic() {
    let c = PlatformPowerController::new();
    // 可能 Ok 也可能 Err（无权限/无接口），但绝不能 panic
    let _ = c.get_profile();
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn capabilities_are_static_strings() {
    let c = PlatformPowerController::new();
    for cap in c.capabilities() {
        assert!(!cap.is_empty());
    }
}
