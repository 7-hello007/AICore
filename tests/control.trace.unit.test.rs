//! ControlTrace / StageOutcome / RollbackRecord 序列化测试。

use aicore_contracts::{
    control::{
        rollback::RollbackRecord,
        stage::{ControlStage, StageOutcome, StageStatus},
        trace::ControlTrace,
    },
    execution::status::ExecutionOverallStatus,
};

#[test]
fn stage_outcome_serde_roundtrip() {
    let s = StageOutcome::success(ControlStage::Apply, 12, "ok");
    let json = serde_json::to_string(&s).unwrap();
    let back: StageOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.stage, ControlStage::Apply);
    assert_eq!(back.status, StageStatus::Success);
}

#[test]
fn stage_status_success_semantics() {
    assert!(StageStatus::Success.is_success());
    assert!(StageStatus::Skipped.is_success());
    assert!(!StageStatus::Failed.is_success());
    assert!(!StageStatus::TimedOut.is_success());
}

#[test]
fn rollback_record_not_triggered_default() {
    let r = RollbackRecord::not_triggered();
    assert!(!r.triggered);
    assert!(r.from_policy_id.is_none());
    assert_eq!(RollbackRecord::default(), r);
}

#[test]
fn rollback_record_triggered_serde() {
    let r = RollbackRecord::triggered(Some("a".to_string()), Some("b".to_string()), "reason", true);
    let json = serde_json::to_string(&r).unwrap();
    let back: RollbackRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r);
    assert!(back.triggered);
    assert_eq!(back.succeeded, Some(true));
}

#[test]
fn control_trace_serde_roundtrip() {
    let trace = ControlTrace {
        trace_id: "t1".to_string(),
        policy_id: "p1".to_string(),
        stages: vec![
            StageOutcome::skipped(ControlStage::Observe, "obs"),
            StageOutcome::success(ControlStage::Apply, 5, "applied"),
        ],
        final_status: ExecutionOverallStatus::AllSucceeded,
        rollback: RollbackRecord::not_triggered(),
        total_duration_ms: 10,
    };
    let json = serde_json::to_string(&trace).unwrap();
    let back: ControlTrace = serde_json::from_str(&json).unwrap();
    assert_eq!(back, trace);
}

#[test]
fn trace_is_success_ignores_rollback_stage() {
    let trace = ControlTrace {
        trace_id: "t".to_string(),
        policy_id: "p".to_string(),
        stages: vec![
            StageOutcome::success(ControlStage::Apply, 0, "ok"),
            StageOutcome::failed(ControlStage::Rollback, 0, "no prior"),
        ],
        final_status: ExecutionOverallStatus::AllFailed,
        rollback: RollbackRecord::not_triggered(),
        total_duration_ms: 0,
    };
    // Rollback 失败不参与 is_success 判定
    assert!(trace.is_success());
}

#[test]
fn trace_stage_lookup() {
    let trace = ControlTrace {
        trace_id: "t".to_string(),
        policy_id: "p".to_string(),
        stages: vec![
            StageOutcome::success(ControlStage::Apply, 0, "ok"),
            StageOutcome::failed(ControlStage::Verify, 0, "bad"),
        ],
        final_status: ExecutionOverallStatus::AllFailed,
        rollback: RollbackRecord::not_triggered(),
        total_duration_ms: 0,
    };
    assert_eq!(
        trace.stage(ControlStage::Apply).unwrap().status,
        StageStatus::Success
    );
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Failed
    );
    assert!(trace.stage(ControlStage::Rollback).is_none());
}
