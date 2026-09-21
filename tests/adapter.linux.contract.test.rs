#![cfg(target_os = "linux")]

use aicore_adapter_linux::LinuxTelemetryProvider;
use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;

#[test]
fn linux_provider_collects_snapshot() {
    let provider = LinuxTelemetryProvider::new();
    let snapshot = provider.collect().expect("collect");

    assert!(snapshot.cpu.logical_cores > 0);
    assert_eq!(provider.platform_name(), "linux");
    assert!(provider.capabilities().contains(&"cpu.telemetry"));
}
