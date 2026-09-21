//! 验证 DaemonStatus 的记录与序列化。

use aicore_contracts::{
    context::workload::WorkloadType,
    execution::status::ExecutionOverallStatus,
    policy::priority::PolicySource,
    runtime::{
        daemon_status::DaemonStatus, mode::RuntimeMode, tick::TickOutcome, STATE_SCHEMA_VERSION,
    },
};

fn make_outcome(tick_id: u64, success: bool, mode: RuntimeMode) -> TickOutcome {
    TickOutcome {
        tick_id,
        trace_id: format!("trace-{}-100", tick_id),
        timestamp_unix_ms: 1_700_000_000,
        duration_ms: 5,
        success,
        mode_after: mode,
        workload_type: WorkloadType::Development,
        policy_id: Some(format!("policy-{}", tick_id)),
        policy_source: Some(PolicySource::RuleBased),
        validation_accepted: true,
        execution_status: Some(if success {
            ExecutionOverallStatus::AllSucceeded
        } else {
            ExecutionOverallStatus::AllFailed
        }),
        message: "ok".to_string(),
        ..Default::default()
    }
}

#[test]
fn new_status_has_schema_version() {
    let s = DaemonStatus::new(1234);
    assert_eq!(s.version, STATE_SCHEMA_VERSION);
    assert_eq!(s.pid, 1234);
    assert_eq!(s.tick_count, 0);
    assert_eq!(s.current_mode, RuntimeMode::Normal);
    assert!(s.last_tick.is_none());
}

#[test]
fn record_success_increments_counters() {
    let mut s = DaemonStatus::new(1);
    s.record_tick(&make_outcome(1, true, RuntimeMode::Normal));
    assert_eq!(s.tick_count, 1);
    assert_eq!(s.successful_ticks, 1);
    assert_eq!(s.failed_ticks, 0);
    assert!(s.last_tick.is_some());
}

#[test]
fn record_failure_updates_mode() {
    let mut s = DaemonStatus::new(1);
    s.record_tick(&make_outcome(1, false, RuntimeMode::Degraded));
    assert_eq!(s.tick_count, 1);
    assert_eq!(s.failed_ticks, 1);
    assert_eq!(s.current_mode, RuntimeMode::Degraded);
}

#[test]
fn status_serializes_roundtrip() {
    let mut s = DaemonStatus::new(42);
    s.record_tick(&make_outcome(1, true, RuntimeMode::Normal));

    let json = serde_json::to_string(&s).unwrap();
    let back: DaemonStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn last_tick_summary_has_trace_id() {
    let mut s = DaemonStatus::new(1);
    s.record_tick(&make_outcome(3, true, RuntimeMode::Normal));
    let last = s.last_tick.as_ref().unwrap();
    assert_eq!(last.tick_id, 3);
    assert!(last.trace_id.contains("trace-3"));
}
