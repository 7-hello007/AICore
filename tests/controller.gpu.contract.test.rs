//! GPU/NPU controller contract test.
//!
//! Current contract:
//! - Status query itself should be successful;
//! - `controllable == false` when the platform does not have a physical control path;
//! - `detail` has to explain why it's uncontrollable.
//! - Capabilities should reflect the capabilities of the current platform that have been declared, not be forced to be empty.

use aicore_contracts::traits::{gpu_controller::IGpuController, npu_controller::INpuController};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::{LinuxGpuController, LinuxNpuController};

#[cfg(target_os = "windows")]
use aicore_adapter_windows::{WindowsGpuController, WindowsNpuController};

#[cfg(target_os = "linux")]
fn gpu() -> LinuxGpuController {
    LinuxGpuController::new()
}

#[cfg(target_os = "windows")]
fn gpu() -> WindowsGpuController {
    WindowsGpuController::new()
}

#[cfg(target_os = "linux")]
fn npu() -> LinuxNpuController {
    LinuxNpuController::new()
}

#[cfg(target_os = "windows")]
fn npu() -> WindowsNpuController {
    WindowsNpuController::new()
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn gpu_control_status_reports_unavailable_control() {
    let controller = gpu();
    let status = controller
        .get_status()
        .expect("GPU status query must succeed");

    assert!(
        !status.controllable,
        "GPU control must not be reported as controllable"
    );

    assert!(
        status
            .detail
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        "unavailable GPU control must provide a detail"
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn gpu_capabilities_are_reported() {
    let controller = gpu();

    assert!(
        !controller.capabilities().is_empty(),
        "GPU controller must report its declared capabilities"
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn npu_control_status_reports_unavailable_control() {
    let controller = npu();
    let status = controller
        .get_status()
        .expect("NPU status query must succeed");

    assert!(
        !status.controllable,
        "NPU control must not be reported as controllable"
    );

    assert!(
        status
            .detail
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        "unavailable NPU control must provide a detail"
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn controller_platform_names_are_set() {
    assert!(matches!(gpu().platform_name(), "linux" | "windows"));
    assert!(matches!(npu().platform_name(), "linux" | "windows"));
}
