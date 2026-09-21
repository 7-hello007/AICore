use aicore_contracts::telemetry::{cpu::CpuSnapshot, snapshot::ComputerSnapshot};
use aicore_core::normalizer::TelemetryNormalizer;

#[test]
fn normalizer_clamps_cpu_usage() {
    let snapshot = ComputerSnapshot {
        cpu: CpuSnapshot {
            usage_percent: 150.0,
            per_core_usage: vec![120.0, -10.0],
            ..Default::default()
        },
        ..Default::default()
    };

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    assert_eq!(state.snapshot.cpu.usage_percent, 100.0);
    assert_eq!(state.snapshot.cpu.per_core_usage[0], 100.0);
    assert_eq!(state.snapshot.cpu.per_core_usage[1], 0.0);
}

#[test]
fn normalizer_marks_cpu_health() {
    let snapshot = ComputerSnapshot {
        cpu: CpuSnapshot {
            logical_cores: 8,
            ..Default::default()
        },
        ..Default::default()
    };

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    assert!(state.health.cpu_available);
}

#[test]
fn normalizer_computes_freshness() {
    let snapshot = ComputerSnapshot {
        collected_at_unix_ms: Some(1_700_000_000_000),
        cpu: CpuSnapshot {
            logical_cores: 8,
            ..Default::default()
        },
        ..Default::default()
    };

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    assert!(state.snapshot.freshness_ms.is_some());
    assert!(state.snapshot.freshness_ms.unwrap() > 0);
}

#[test]
fn normalizer_handles_missing_collected_at() {
    let snapshot = ComputerSnapshot::default();

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    assert!(state.snapshot.freshness_ms.is_none());
}
