//! Validate real collection extension.
//!
//! The test must also be passed on the container / VM: Noon, no Panic if you cannot read it.

use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn collect_returns_snapshot_without_panic() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    // No Panic; CPU/ memory must exist
    assert!(s.cpu.logical_cores > 0);
    assert!(s.memory.total_mb > 0);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn temperature_is_optional() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    // Container / VM may not read the temperature; no Panic passes
    let _ = s.temperature.cpu_celsius;
    let _ = s.temperature.gpu_celsius;
    let _ = s.temperature.system_celsius;
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn gpu_is_optional() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    // The GPU field on a non-specified machine is all None or zero; no panic passes
    if let Some(v) = s.gpu.usage_percent {
        assert!((0.0..=100.0).contains(&v), "gpu usage out of range: {}", v);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn npu_is_optional() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    let _ = s.npu.available;
}

#[cfg(target_os = "linux")]
#[test]
fn linux_reports_extended_capabilities() {
    let p = PlatformProvider::new();
    let caps = p.capabilities();
    assert!(caps.contains(&"temperature.telemetry"));
    assert!(caps.contains(&"gpu.telemetry.best_effort"));
    assert!(caps.contains(&"foreground.telemetry.best_effort"));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn sampling_constraints_are_non_negative() {
    let p = PlatformProvider::new();
    let c = p.sampling_constraints();
    // Default > = 0
    assert!(c.min_interval_ms < 10_000, "constraint unexpectedly large");
    assert!(!c.should_skip(0) || c.min_interval_ms > 0);
}
