#![cfg(target_os = "windows")]

use aicore_adapter_windows::WindowsTelemetryProvider;
use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;

#[test]
fn windows_provider_collects_snapshot() {
    let provider = WindowsTelemetryProvider::new();
    let snapshot = provider.collect().expect("collect");

    assert!(snapshot.cpu.logical_cores > 0);
    assert_eq!(provider.platform_name(), "windows");
    assert!(provider.capabilities().contains(&"cpu.telemetry"));
}
