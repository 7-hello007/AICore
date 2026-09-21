//! TickHistory + GetRecentTicks 协议测试。

use aicore_contracts::{
    context::workload::WorkloadType,
    execution::status::ExecutionOverallStatus,
    ipc::protocol::{Request, ResponseData},
    policy::priority::PolicySource,
    runtime::{daemon_status::TickSummary, mode::RuntimeMode, tick::TickOutcome},
};

fn sample_tick(id: u64) -> TickOutcome {
    TickOutcome {
        tick_id: id,
        trace_id: format!("trace-{}-100", id),
        timestamp_unix_ms: 1_700_000_000 + id,
        duration_ms: 5,
        success: true,
        mode_after: RuntimeMode::Normal,
        workload_type: WorkloadType::Development,
        policy_id: Some(format!("policy-{}", id)),
        policy_source: Some(PolicySource::RuleBased),
        validation_accepted: true,
        execution_status: Some(ExecutionOverallStatus::AllSucceeded),
        message: "ok".to_string(),
        ..Default::default()
    }
}

#[test]
fn request_recent_ticks_serializes() {
    let req = Request::GetRecentTicks { limit: 5 };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("get_recent_ticks"));
    assert!(json.contains('5'));
    let back: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(back, req);
}

#[test]
fn response_recent_ticks_serializes() {
    let tick = TickSummary::from_outcome(&sample_tick(1));
    let resp = ResponseData::RecentTicks {
        ticks: vec![tick.clone()],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("recent_ticks"));
    assert!(json.contains("trace-1-100"));
    let back: ResponseData = serde_json::from_str(&json).unwrap();
    assert_eq!(back, resp);
}

#[test]
fn tick_summary_from_outcome_includes_prediction() {
    let mut o = sample_tick(7);
    o.predicted_workload = Some(WorkloadType::AiInference);
    o.prediction_confidence = Some(0.78);

    let s = TickSummary::from_outcome(&o);
    assert_eq!(s.predicted_workload, Some(WorkloadType::AiInference));
    assert_eq!(s.prediction_confidence, Some(0.78));
}

#[test]
fn tick_summary_without_prediction_omits_fields() {
    let o = sample_tick(8);
    let s = TickSummary::from_outcome(&o);
    let json = serde_json::to_string(&s).unwrap();
    // skip_serializing_if 生效
    assert!(!json.contains("predicted_workload"));
    assert!(!json.contains("prediction_confidence"));
}
