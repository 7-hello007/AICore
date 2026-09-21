//! 验证真实采集扩展。
//!
//! 测试在容器 / VM 上也必须通过：读不到就返回 None，不 panic。

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
    // 不 panic；CPU / memory 必须存在
    assert!(s.cpu.logical_cores > 0);
    assert!(s.memory.total_mb > 0);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn temperature_is_optional() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    // 容器 / VM 可能读不到温度；不 panic 即通过
    let _ = s.temperature.cpu_celsius;
    let _ = s.temperature.gpu_celsius;
    let _ = s.temperature.system_celsius;
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn gpu_is_optional() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    // 无独显机器上 GPU 字段全是 None 或零值；不 panic 即通过
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
    // 默认 >= 0
    assert!(c.min_interval_ms < 10_000, "constraint unexpectedly large");
    assert!(!c.should_skip(0) || c.min_interval_ms > 0);
}
