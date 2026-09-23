//! Render 纯函数测试。

use aicore_contracts::{
    capability::entry::CapabilityEntry,
    runtime::{
        daemon_status::{DaemonStatus, TickSummary},
        mode::RuntimeMode,
    },
};
use aicore_dashboard::{render, DashboardSnapshot};

fn empty_snapshot() -> DashboardSnapshot {
    DashboardSnapshot {
        fetched_at_unix_ms: 1_700_000_000_000,
        ..Default::default()
    }
}

#[test]
fn empty_snapshot_renders_all_sections() {
    let s = empty_snapshot();
    let out = render(&s, 80);

    assert!(out.contains("AICore Dashboard"));
    assert!(out.contains("[ Status ]"));
    assert!(out.contains("(daemon unavailable)"));
    assert!(out.contains("[ Recent Ticks ]"));
    assert!(out.contains("(none)"));
    assert!(out.contains("[ Capabilities ]"));
    assert!(out.contains("[ Plugins ]"));
    assert!(out.contains("status: OK"));
}

#[test]
fn status_renders_daemon_info() {
    let mut s = empty_snapshot();
    let mut st = DaemonStatus::new(12345);
    st.tick_count = 42;
    st.successful_ticks = 40;
    st.failed_ticks = 2;
    st.current_mode = RuntimeMode::Normal;
    st.last_tick_at_unix_ms = 1_700_000_000_000;
    s.daemon_status = Some(st);

    let out = render(&s, 100);

    assert!(out.contains("pid"));
    assert!(out.contains("12345"));
    assert!(out.contains("42"));
    assert!(out.contains("40 / 2"));
    assert!(out.contains("Normal"));
}

#[test]
fn recent_ticks_table_renders_rows() {
    let mut s = empty_snapshot();
    let mut st = DaemonStatus::new(1);
    st.last_tick = None;
    s.daemon_status = Some(st);

    s.recent_ticks = vec![TickSummary {
        tick_id: 7,
        trace_id: "trace-7".to_string(),
        timestamp_unix_ms: 1_700_000_000_000,
        duration_ms: 12,
        success: true,
        workload_type: "Development".to_string(),
        policy_id: Some("p-7".to_string()),
        policy_source: Some(aicore_contracts::policy::priority::PolicySource::RuleBased),
        execution_status: None,
        message: "ok".to_string(),
        predicted_workload: Some(aicore_contracts::context::workload::WorkloadType::AiInference),
        prediction_confidence: Some(0.85),
        ..Default::default()
    }];

    let out = render(&s, 100);
    assert!(out.contains("tick"));
    assert!(out.contains("Development"));
    assert!(out.contains("0.85"));
}

#[test]
fn capabilities_render_status_and_detail() {
    let mut s = empty_snapshot();
    s.capabilities = vec![
        CapabilityEntry::supported("cpu.telemetry"),
        CapabilityEntry::unsupported(
            "gpu.control",
            "capability depends on hardware vendor adapter",
        ),
    ];

    let out = render(&s, 100);
    assert!(out.contains("Supported"));
    assert!(out.contains("cpu.telemetry"));
    assert!(out.contains("Unsupported"));
    assert!(out.contains("hardware vendor adapter"));
}

#[test]
fn plugins_render_metadata() {
    use aicore_contracts::ipc::protocol::PluginEntryDto;

    let mut s = empty_snapshot();
    s.plugin_states = vec![PluginEntryDto {
        name: "example".to_string(),
        version: "1.0.0".to_string(),
        api_version: "1".to_string(),
        kind: "Adapter".to_string(),
        trust_level: "Trusted".to_string(),
        state: "Active".to_string(),
        interfaces: vec!["IGpuController".to_string()],
        permissions: vec![],
        last_reason: None,
        failure_count: 0,
    }];

    let out = render(&s, 100);
    assert!(out.contains("example"));
    assert!(out.contains("1.0.0"));
    assert!(out.contains("IGpuController"));
}

#[test]
fn errors_render_when_present() {
    let mut s = empty_snapshot();
    s.errors.push("GetStatus: connect refused".to_string());

    let out = render(&s, 100);
    assert!(out.contains("[ Errors ]"));
    assert!(out.contains("connect refused"));
    assert!(out.contains("1 errors"));
}

#[test]
fn width_respected_minimum() {
    // 极窄宽度应该被 clamp 到 60
    let s = empty_snapshot();
    let out = render(&s, 10);
    assert!(out.contains('═'));
    // 至少第一行 bar 足够宽
    let first_line = out.lines().next().unwrap();
    assert!(first_line.chars().count() >= 60);
}

#[test]
fn snapshot_health_helpers() {
    let mut s = empty_snapshot();
    assert!(!s.is_healthy());

    s.daemon_status = Some(DaemonStatus::new(1));
    assert!(s.is_healthy());

    s.errors.push("x".to_string());
    assert!(!s.is_healthy());
    assert_eq!(s.total_errors(), 1);
}
