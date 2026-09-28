//! Authentication Capability Provider Returns a real, structured list of capabilities.

use aicore_contracts::{
    capability::status::CapabilityStatus, traits::capability_provider::ICapabilityProvider,
};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxCapabilityProvider as PlatformCapabilityProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsCapabilityProvider as PlatformCapabilityProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn provider_returns_non_empty_capabilities() {
    let p = PlatformCapabilityProvider::new();
    let caps = p.capabilities_detailed();
    assert!(!caps.is_empty(), "capability list must not be empty");
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn cpu_memory_process_telemetry_always_supported() {
    let p = PlatformCapabilityProvider::new();
    let names = p.capability_names();

    assert!(names.contains(&"cpu.telemetry".to_string()));
    assert!(names.contains(&"memory.telemetry".to_string()));
    assert!(names.contains(&"disk.telemetry".to_string()));
    assert!(names.contains(&"network.telemetry".to_string()));
    assert!(names.contains(&"process.telemetry".to_string()));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn all_entries_are_well_formed() {
    let p = PlatformCapabilityProvider::new();
    for cap in p.capabilities_detailed() {
        assert!(
            cap.is_well_formed(),
            "capability {} with status {:?} must have non-empty detail",
            cap.name,
            cap.status
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn unsupported_capabilities_have_detail() {
    let p = PlatformCapabilityProvider::new();
    for cap in p.capabilities_detailed() {
        if cap.status == CapabilityStatus::Unsupported {
            assert!(
                cap.detail
                    .as_deref()
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false),
                "unsupported capability {} must have non-empty detail",
                cap.name
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn component_name_is_platform_specific() {
    let p = PlatformCapabilityProvider::new();
    let name = p.component_name();
    assert!(
        name == "linux_capability" || name == "windows_capability",
        "unexpected component name: {}",
        name
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn capability_lookup_by_name_works() {
    let p = PlatformCapabilityProvider::new();

    let cpu = p.capability("cpu.telemetry");
    assert!(cpu.is_some(), "cpu.telemetry should be present");
    assert_eq!(cpu.unwrap().status, CapabilityStatus::Supported);

    let missing = p.capability("nonexistent.capability");
    assert!(missing.is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_declares_process_write_conditional_on_current_permissions() {
    let p = PlatformCapabilityProvider::new();
    let cap = p
        .capability("process.priority.write")
        .expect("linux process.priority.write present");
    assert_eq!(cap.status, CapabilityStatus::Conditional);
    assert!(cap
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("UID")));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_declares_process_write_conditional_on_target_rights() {
    let p = PlatformCapabilityProvider::new();
    let cap = p
        .capability("process.priority.write")
        .expect("windows process.priority.write present");
    assert_eq!(cap.status, CapabilityStatus::Conditional);
    assert!(cap
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("target PID")));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn no_duplicate_capability_names() {
    let p = PlatformCapabilityProvider::new();
    let mut names = p.capability_names();
    let before = names.len();
    names.sort();
    names.dedup();
    assert_eq!(before, names.len(), "capability names must be unique");
}
