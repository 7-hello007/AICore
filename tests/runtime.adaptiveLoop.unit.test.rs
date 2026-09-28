use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    runtime::{config::AdaptiveLoopConfig, mode::RuntimeMode},
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::RuleBasedPolicyEngine,
};
use aicore_runtime::AdaptiveLoop;

/// Mark Telemetry Provider.
struct MockTelemetry {
    fail: bool,
    calls: AtomicU64,
}

impl MockTelemetry {
    fn ok() -> Self {
        Self {
            fail: false,
            calls: AtomicU64::new(0),
        }
    }
    fn failing() -> Self {
        Self {
            fail: true,
            calls: AtomicU64::new(0),
        }
    }
}

impl ITelemetryProvider for MockTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(ContractError::ResourceUnavailable(
                "mock failure".to_string(),
            ));
        }
        let mut snapshot = ComputerSnapshot::default();
        snapshot.cpu.logical_cores = 8;
        Ok(snapshot)
    }
    fn platform_name(&self) -> &'static str {
        "mock"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["cpu.telemetry"]
    }
}

/// Executor that always returns `AllSucceeded`.
struct OkExecutor;

impl IPolicyExecutor for OkExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "test".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::Success,
                message: "ok".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "ok_executor"
    }
}

/// Executor that always returns `AllFailed`.
struct FailExecutor;

impl IPolicyExecutor for FailExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllFailed,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "test".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::Failed,
                message: "fail".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "fail_executor"
    }
}

fn build_loop(
    telemetry: Arc<dyn ITelemetryProvider>,
    executor: Arc<dyn IPolicyExecutor>,
    config: AdaptiveLoopConfig,
) -> AdaptiveLoop {
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let primary: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    AdaptiveLoop::with_default_validator(telemetry, ctx_engine, primary, fallback, executor, config)
}

#[test]
fn tick_with_success_returns_outcome() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry.clone(), executor, AdaptiveLoopConfig::default());

    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(outcome.success);
    assert_eq!(outcome.mode_after, RuntimeMode::Normal);
    assert_eq!(outcome.workload_type, WorkloadType::Unknown);
    assert!(outcome.validation_accepted);
    assert!(outcome.policy_id.is_some());
    assert_eq!(telemetry.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn metrics_accumulate_over_ticks() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    for _ in 0..3 {
        loop_.tick(&ExecutionTargets::default());
    }

    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
    assert_eq!(m.successful_ticks, 3);
    assert_eq!(m.policies_generated, 3);
    assert_eq!(m.executions_succeeded, 3);
    assert!(m.total_tick_duration_ms > 0 || m.last_tick_duration_ms == 0);
}

#[test]
fn telemetry_failure_marks_tick_failed() {
    let telemetry = Arc::new(MockTelemetry::failing());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(!outcome.success);
    assert!(outcome.policy_id.is_none());
    assert!(!outcome.validation_accepted);
    assert_eq!(loop_.current_mode(), RuntimeMode::Normal); // Single failure is still in Normal
}

#[test]
fn repeated_executor_failures_enter_degraded() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(FailExecutor);
    let config = AdaptiveLoopConfig {
        degraded_threshold: 2,
        safe_threshold: 4,
        recovery_threshold: 2,
    };
    let loop_ = build_loop(telemetry, executor, config);

    loop_.tick(&ExecutionTargets::default());
    assert_eq!(loop_.current_mode(), RuntimeMode::Normal);
    loop_.tick(&ExecutionTargets::default());
    assert_eq!(loop_.current_mode(), RuntimeMode::Degraded);
    loop_.tick(&ExecutionTargets::default());
    loop_.tick(&ExecutionTargets::default());
    assert_eq!(loop_.current_mode(), RuntimeMode::Safe);
}

#[test]
fn loop_reports_name() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());
    assert_eq!(loop_.name(), "adaptive_loop");
}

#[test]
fn tick_ids_increment() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    let o1 = loop_.tick(&ExecutionTargets::default());
    let o2 = loop_.tick(&ExecutionTargets::default());
    let o3 = loop_.tick(&ExecutionTargets::default());

    assert_eq!(o1.tick_id, 1);
    assert_eq!(o2.tick_id, 2);
    assert_eq!(o3.tick_id, 3);
}

#[test]
fn tick_outcome_has_trace_id() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(!outcome.trace_id.is_empty());
    assert!(outcome.trace_id.starts_with("trace-"));
    assert!(outcome.trace_id.contains(&outcome.tick_id.to_string()));
}

#[test]
fn consecutive_ticks_have_different_trace_ids() {
    let telemetry = Arc::new(MockTelemetry::ok());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    let o1 = loop_.tick(&ExecutionTargets::default());
    let o2 = loop_.tick(&ExecutionTargets::default());

    assert_ne!(o1.trace_id, o2.trace_id);
    assert_ne!(o1.tick_id, o2.tick_id);
}

#[test]
fn failed_tick_still_has_trace_id() {
    let telemetry = Arc::new(MockTelemetry::failing());
    let executor = Arc::new(OkExecutor);
    let loop_ = build_loop(telemetry, executor, AdaptiveLoopConfig::default());

    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(!outcome.success);
    assert!(!outcome.trace_id.is_empty());
    assert!(outcome.trace_id.starts_with("trace-"));
}

#[test]
fn rejected_policy_finishes_dequeued_task_as_failed() {
    use aicore_contracts::{
        application::id::ApplicationId,
        task::{contract::TaskContract, lifecycle::TaskState, queued::QueuedTask},
        traits::{
            policy_validator::{IPolicyValidator, ValidationIssue, ValidationResult},
            task_queue::ITaskQueue,
        },
    };
    struct RejectValidator;
    impl IPolicyValidator for RejectValidator {
        fn validate(&self, _: &ResourcePolicy) -> ValidationResult {
            ValidationResult {
                accepted: false,
                issues: vec![ValidationIssue {
                    code: "ERR_POLICY_INVALID".into(),
                    message: "regression rejection".into(),
                }],
            }
        }
        fn name(&self) -> &'static str {
            "reject_validator"
        }
    }
    struct NeverExecute;
    impl IPolicyExecutor for NeverExecute {
        fn execute(&self, _: &ResourcePolicy, _: &ExecutionTargets) -> ExecutionResult {
            panic!("rejected policy must never execute")
        }
        fn name(&self) -> &'static str {
            "never_execute"
        }
    }
    let queue = Arc::new(aicore_task_runtime::InMemoryTaskQueue::new());
    queue
        .enqueue(QueuedTask::new(
            "rejected-task".into(),
            TaskContract::minimal(ApplicationId::new("test.app")),
            0,
        ))
        .unwrap();
    let engine = Arc::new(RuleBasedPolicyEngine::new());
    let loop_ = AdaptiveLoop::new(
        Arc::new(MockTelemetry::ok()),
        Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
        engine.clone(),
        engine,
        Arc::new(RejectValidator),
        Arc::new(NeverExecute),
        AdaptiveLoopConfig::default(),
    )
    .with_task_queue(queue.clone());
    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(!outcome.validation_accepted);
    let task = queue.get("rejected-task").unwrap().unwrap();
    assert_eq!(task.state, TaskState::Failed);
    assert!(task.finished_at_unix_ms.is_some());
    assert!(task.result_message.unwrap().contains("policy rejected"));
}
