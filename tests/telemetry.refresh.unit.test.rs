//! 验证 Telemetry Provider 每次 collect 都返回新鲜数据。

use std::thread;
use std::time::Duration;

use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn two_collects_return_different_timestamps() {
    let p = PlatformProvider::new();
    let s1 = p.collect().expect("collect 1");

    thread::sleep(Duration::from_millis(5));

    let s2 = p.collect().expect("collect 2");

    let t1 = s1.collected_at_unix_ms.expect("t1");
    let t2 = s2.collected_at_unix_ms.expect("t2");

    assert!(
        t2 > t1,
        "expected second collect to have later timestamp; got t1={} t2={}",
        t1,
        t2
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn collected_at_is_set() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    assert!(s.collected_at_unix_ms.is_some());
    assert!(s.collected_at_unix_ms.unwrap() > 0);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn snapshot_has_cpu_cores() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    assert!(s.cpu.logical_cores > 0);
    assert!(!s.cpu.per_core_usage.is_empty());
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn snapshot_has_memory() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    assert!(s.memory.total_mb > 0);
    assert!(s.memory.used_mb <= s.memory.total_mb);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn snapshot_has_disks_or_is_empty() {
    let p = PlatformProvider::new();
    let s = p.collect().expect("collect");
    for d in &s.disks {
        assert!(d.total_gb >= 0.0);
        assert!(d.usage_percent >= 0.0 && d.usage_percent <= 100.0);
    }
}
