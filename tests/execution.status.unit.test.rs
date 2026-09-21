//! ExecutionOverallStatus 语义测试。

use aicore_contracts::execution::{
    result::ExecutionOutcome,
    status::{ExecutionOverallStatus, ExecutionStatus},
};

fn outcome(status: ExecutionStatus) -> ExecutionOutcome {
    ExecutionOutcome {
        resource: "test".to_string(),
        target: "system".to_string(),
        status,
        message: "test".to_string(),
    }
}

#[test]
fn empty_is_noop() {
    let o = ExecutionOverallStatus::from_outcomes(&[]);
    assert_eq!(o, ExecutionOverallStatus::NoOp);
}

#[test]
fn all_success_is_all_succeeded() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Success),
        outcome(ExecutionStatus::Success),
    ]);
    assert_eq!(o, ExecutionOverallStatus::AllSucceeded);
    assert!(o.is_success());
    assert!(!o.is_failure());
}

#[test]
fn all_not_supported_is_noop_not_failure() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::NotSupported),
        outcome(ExecutionStatus::NotSupported),
    ]);
    assert_eq!(o, ExecutionOverallStatus::NoOp);
    assert!(!o.is_success());
    assert!(!o.is_failure(), "NotSupported must NOT count as failure");
}

#[test]
fn all_skipped_is_noop() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Skipped),
        outcome(ExecutionStatus::Skipped),
    ]);
    assert_eq!(o, ExecutionOverallStatus::NoOp);
    assert!(!o.is_failure());
}

#[test]
fn mixed_skipped_and_not_supported_is_noop() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Skipped),
        outcome(ExecutionStatus::NotSupported),
    ]);
    assert_eq!(o, ExecutionOverallStatus::NoOp);
}

#[test]
fn real_failure_counts() {
    let o = ExecutionOverallStatus::from_outcomes(&[outcome(ExecutionStatus::Failed)]);
    assert_eq!(o, ExecutionOverallStatus::AllFailed);
    assert!(o.is_failure());
}

#[test]
fn permission_denied_counts_as_real_failure() {
    let o = ExecutionOverallStatus::from_outcomes(&[outcome(ExecutionStatus::PermissionDenied)]);
    assert_eq!(o, ExecutionOverallStatus::AllFailed);
    assert!(o.is_failure());
}

#[test]
fn partial_success_wins_over_noop() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Success),
        outcome(ExecutionStatus::NotSupported),
    ]);
    assert_eq!(o, ExecutionOverallStatus::PartiallySucceeded);
    assert!(o.is_success());
    assert!(!o.is_failure());
}

#[test]
fn success_and_failure_is_partial() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Success),
        outcome(ExecutionStatus::Failed),
    ]);
    assert_eq!(o, ExecutionOverallStatus::PartiallySucceeded);
    assert!(o.is_success());
}

#[test]
fn no_success_with_failures_is_all_failed() {
    let o = ExecutionOverallStatus::from_outcomes(&[
        outcome(ExecutionStatus::Failed),
        outcome(ExecutionStatus::PermissionDenied),
        outcome(ExecutionStatus::Skipped),
    ]);
    assert_eq!(o, ExecutionOverallStatus::AllFailed);
    assert!(o.is_failure());
}

#[test]
fn status_is_real_failure_semantics() {
    assert!(ExecutionStatus::Failed.is_real_failure());
    assert!(ExecutionStatus::PermissionDenied.is_real_failure());
    assert!(!ExecutionStatus::NotSupported.is_real_failure());
    assert!(!ExecutionStatus::Skipped.is_real_failure());
    assert!(!ExecutionStatus::Success.is_real_failure());
}
