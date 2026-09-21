use aicore_contracts::telemetry::ComputerSnapshot;

#[test]
fn snapshot_default_is_serializable() {
    let snapshot = ComputerSnapshot::default();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    assert!(json.contains("cpu"));
    assert!(json.contains("memory"));
}

#[test]
fn snapshot_roundtrip() {
    let snapshot = ComputerSnapshot::default();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let back: ComputerSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(snapshot.cpu.logical_cores, back.cpu.logical_cores);
}
