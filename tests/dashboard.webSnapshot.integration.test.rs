//! Terminal 与 Web 共用 DashboardSnapshot 的静态一致性测试。

use aicore_contracts::dashboard::snapshot::DashboardSnapshot;

#[test]
fn snapshot_has_all_v3_fields() {
    let snap = DashboardSnapshot::default();
    // 15 字段；缺失字段会导致下列断言不通过
    let _ = &snap.daemon_status;
    let _ = &snap.current_context;
    let _ = &snap.current_workload;
    let _ = &snap.prediction;
    let _ = &snap.current_policy;
    let _ = &snap.runtime_route;
    let _ = &snap.execution_status;
    let _ = &snap.rollback_status;
    let _ = &snap.capabilities;
    let _ = &snap.tier_states;
    let _ = &snap.plugin_states;
    let _ = &snap.retention_status;
    let _ = &snap.ai_runtime_health;
    let _ = &snap.recent_ticks;
    let _ = &snap.errors;
}

#[test]
fn snapshot_serializes_to_json() {
    let snap = DashboardSnapshot::new(1_700_000_000);
    let json = serde_json::to_string(&snap).unwrap();
    assert!(json.contains("schema_version"));
    assert!(json.contains("fetched_at_unix_secs"));

    let back: DashboardSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, snap);
}
