//! TransparentExecutor 六段式测试。

use std::sync::Arc;

use aicore_contracts::{
    control::stage::{ControlStage, StageStatus},
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    traits::{
        policy_executor::IPolicyExecutor,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
        transparent_executor::ITransparentExecutor,
    },
};
use aicore_core::executor::TransparentExecutor;

/// Mock executor：可配置返回状态。
struct MockExecutor {
    status: ExecutionOverallStatus,
}

impl IPolicyExecutor for MockExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: self.status,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "mock".to_string(),
                target: "system".to_string(),
                status: match self.status {
                    ExecutionOverallStatus::AllSucceeded => ExecutionStatus::Success,
                    ExecutionOverallStatus::AllFailed => ExecutionStatus::Failed,
                    _ => ExecutionStatus::Skipped,
                },
                message: "mock".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "mock_executor"
    }
}

/// 可配置的 checker。
struct MockChecker {
    result: PostConditionResult,
}

impl IPostConditionChecker for MockChecker {
    fn verify(&self, _p: &ResourcePolicy, _r: &ExecutionResult) -> PostConditionResult {
        self.result.clone()
    }
    fn checker_name(&self) -> &'static str {
        "mock_checker"
    }
}

fn policy(id: &str) -> ResourcePolicy {
    ResourcePolicy {
        policy_id: id.to_string(),
        priority: PolicyPriority::Normal,
        source: PolicySource::RuleBased,
        reason: "test".to_string(),
        cpu: CpuPolicy {
            priority: PolicyPriority::Normal,
            affinity_cores: None,
        },
        gpu: GpuPolicy {
            priority: PolicyPriority::Normal,
            workload_preference: None,
        },
        npu: NpuPolicy {
            priority: PolicyPriority::Normal,
            preferred: false,
        },
        memory: MemoryPolicy {
            priority: PolicyPriority::Normal,
            max_usage_percent: None,
        },
        power: PowerPolicy {
            mode: PowerMode::Balanced,
        },
        background: BackgroundPolicy {
            throttle: false,
            max_cpu_percent: None,
        },
        ..Default::default()
    }
}

#[test]
fn success_path_produces_six_stages() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("p1"), &ExecutionTargets::default(), None);

    // 6 阶段：Observe / Decide / Validate / Apply / Verify / Rollback
    assert_eq!(trace.stages.len(), 6);
    assert_eq!(
        trace.stage(ControlStage::Observe).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Decide).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Validate).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Apply).unwrap().status,
        StageStatus::Success
    );
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Rollback).unwrap().status,
        StageStatus::Skipped
    );

    assert!(!trace.rollback.triggered);
    assert!(trace.is_success());
    assert_eq!(tx.name(), "transparent_executor");
}

#[test]
fn apply_failure_triggers_rollback_with_prior() {
    // inner 在首次调用返回 AllFailed，回退时返回 AllSucceeded。
    // 用状态机模拟。
    use std::sync::atomic::{AtomicU32, Ordering};
    struct SwitchingExecutor {
        call_count: AtomicU32,
    }
    impl IPolicyExecutor for SwitchingExecutor {
        fn execute(&self, p: &ResourcePolicy, _t: &ExecutionTargets) -> ExecutionResult {
            let n = self.call_count.fetch_add(1, Ordering::SeqCst);
            let status = if n == 0 {
                ExecutionOverallStatus::AllFailed
            } else {
                ExecutionOverallStatus::AllSucceeded
            };
            ExecutionResult {
                policy_id: p.policy_id.clone(),
                overall_status: status,
                started_at_unix_ms: 0,
                finished_at_unix_ms: 0,
                outcomes: Vec::new(),
            }
        }
        fn name(&self) -> &'static str {
            "switching_executor"
        }
    }

    let inner = Arc::new(SwitchingExecutor {
        call_count: AtomicU32::new(0),
    });
    let tx = TransparentExecutor::new(inner);

    let prior = policy("prior");
    let trace = tx.execute_transparent(&policy("new"), &ExecutionTargets::default(), Some(&prior));

    assert!(trace.rollback.triggered);
    assert_eq!(trace.rollback.succeeded, Some(true));
    assert_eq!(trace.rollback.to_policy_id.as_deref(), Some("prior"));
    // Apply 阶段记录成功（因为 execute 被调用），但 Verify 标记失败
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Failed
    );
    assert_eq!(
        trace.stage(ControlStage::Rollback).unwrap().status,
        StageStatus::Success
    );
}

#[test]
fn apply_failure_without_prior_records_failed_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllFailed,
    });
    let tx = TransparentExecutor::new(inner);

    let trace = tx.execute_transparent(&policy("new"), &ExecutionTargets::default(), None);

    assert!(trace.rollback.triggered);
    assert_eq!(trace.rollback.succeeded, Some(false));
    assert!(trace.rollback.to_policy_id.is_none());
    assert_eq!(
        trace.stage(ControlStage::Rollback).unwrap().status,
        StageStatus::Failed
    );
}

#[test]
fn noop_does_not_trigger_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::NoOp,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("p"), &ExecutionTargets::default(), None);

    assert!(!trace.rollback.triggered);
    assert!(!trace.is_success());
    // NoOp → Verify 不适用
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
}

#[test]
fn checker_failure_triggers_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let checker = Arc::new(MockChecker {
        result: PostConditionResult::Failed("state mismatch".to_string()),
    });
    let tx = TransparentExecutor::new(inner).with_checker(checker);
    assert!(tx.has_checker());

    let prior = policy("prior");
    let trace = tx.execute_transparent(&policy("new"), &ExecutionTargets::default(), Some(&prior));

    assert!(trace.rollback.triggered);
    let verify = trace.stage(ControlStage::Verify).unwrap();
    assert_eq!(verify.status, StageStatus::Failed);
    assert!(verify.detail.contains("state mismatch"));
}

#[test]
fn checker_not_applicable_is_not_failure() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let checker = Arc::new(MockChecker {
        result: PostConditionResult::NotApplicable,
    });
    let tx = TransparentExecutor::new(inner).with_checker(checker);
    let trace = tx.execute_transparent(&policy("p"), &ExecutionTargets::default(), None);

    assert!(!trace.rollback.triggered);
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
}

#[test]
fn trace_carries_policy_id_and_duration() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("my-policy"), &ExecutionTargets::default(), None);

    assert_eq!(trace.policy_id, "my-policy");
    assert!(trace.trace_id.contains("my-policy"));
    // duration >= 0
    assert!(trace.total_duration_ms < 10_000);
}
