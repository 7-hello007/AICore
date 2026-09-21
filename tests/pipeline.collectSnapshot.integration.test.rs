use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;
use aicore_core::normalizer::TelemetryNormalizer;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn collect_then_normalize() {
    let provider = PlatformProvider::new();
    let snapshot = provider.collect().expect("collect");

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    assert!(state.health.cpu_available);
    assert!(state.snapshot.cpu.usage_percent >= 0.0);
    assert!(state.snapshot.cpu.usage_percent <= 100.0);
}
