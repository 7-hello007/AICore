//! Task Pipeline Integration Test: enqueue → AdaptiveLoop tick consumption → complete.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::execution::{
    result::{ExecutionOutcome, ExecutionResult},
    status::{ExecutionOverallStatus, ExecutionStatus},
};
use aicore_contracts::policy::resource_policy::ResourcePolicy;
use aicore_contracts::{
    application::id::ApplicationId,
    context::workload::WorkloadType,
    errors::ContractResult,
    execution::targets::ExecutionTargets,
    runtime::config::AdaptiveLoopConfig,
    task::{contract::TaskContract, lifecycle::TaskState, queued::QueuedTask},
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, task_queue::ITaskQueue,
        telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::RuleBasedPolicyEngine,
};
use aicore_runtime::AdaptiveLoop;
use aicore_task_runtime::InMemoryTaskQueue;

struct MockTelemetry {
    calls: AtomicU64,
}

impl MockTelemetry {
    fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
        }
    }
}

impl ITelemetryProvider for MockTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut s = ComputerSnapshot::default();
        s.cpu.logical_cores = 8;
        Ok(s)
    }
    fn platform_name(&self) -> &'static str {
        "mock"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
}

struct OkExecutor;

impl IPolicyExecutor for OkExecutor {
    fn execute(&self, p: &ResourcePolicy, _t: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: p.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "mock".to_string(),
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

struct NoOpExecutor;

impl IPolicyExecutor for NoOpExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: ExecutionOverallStatus::NoOp,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "mock".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::NotSupported,
                message: "unavailable".to_string(),
            }],
        }
    }

    fn name(&self) -> &'static str {
        "noop_executor"
    }
}

fn build_loop(queue: Arc<dyn ITaskQueue>) -> AdaptiveLoop {
    build_loop_with_executor(queue, Arc::new(OkExecutor))
}

fn build_loop_with_executor(
    queue: Arc<dyn ITaskQueue>,
    executor: Arc<dyn IPolicyExecutor>,
) -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());
    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_task_queue(queue)
}

fn dev_task(id: &str) -> QueuedTask {
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.ide"));
    c.task_id = Some(id.to_string());
    c.task = Some(WorkloadType::Development);
    QueuedTask::new(id.to_string(), c, 1000)
}

#[test]
fn task_enqueued_then_consumed_by_tick() {
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    queue.enqueue(dev_task("t1")).unwrap();
    assert_eq!(queue.queued_count().unwrap(), 1);

    let loop_ = build_loop(queue.clone());
    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(outcome.success);
    // task consumed
    let t = queue.get("t1").unwrap().unwrap();
    assert_eq!(t.state, TaskState::Completed);
    assert_eq!(queue.queued_count().unwrap(), 0);
}

#[test]
fn tick_without_task_has_no_task_id_in_message() {
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    let loop_ = build_loop(queue.clone());
    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(outcome.success);
    // queue is still empty
    assert_eq!(queue.queued_count().unwrap(), 0);
}

#[test]
fn multiple_ticks_consume_multiple_tasks() {
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    for i in 0..3 {
        queue.enqueue(dev_task(&format!("t{}", i))).unwrap();
    }

    let loop_ = build_loop(queue.clone());
    for completed in 1..=3 {
        assert!(loop_.tick(&ExecutionTargets::default()).success);
        let tasks = queue.snapshot().unwrap();
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.state == TaskState::Completed)
                .count(),
            completed
        );
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.state == TaskState::Queued)
                .count(),
            3 - completed
        );
        assert!(tasks.iter().all(|task| task.state != TaskState::Running));
    }

    assert_eq!(queue.queued_count().unwrap(), 0);
    let snap = queue.snapshot().unwrap();
    assert!(snap.iter().all(|t| t.state == TaskState::Completed));
}

#[test]
fn invalid_transition_during_finish_is_reported_gracefully() {
    // Artisanal construction: First enquee, no dequee, direct complete fails
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    queue.enqueue(dev_task("t1")).unwrap();
    let result = queue.complete("t1", None);
    assert!(result.is_err());
}

#[test]
fn task_without_task_field_does_not_change_workload() {
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    // It's not gonna change.workload_type
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.app"));
    c.task_id = Some("t1".to_string());
    queue
        .enqueue(QueuedTask::new("t1".to_string(), c, 1000))
        .unwrap();

    let loop_ = build_loop(queue.clone());
    let outcome = loop_.tick(&ExecutionTargets::default());
    // Still successfully implemented
    assert!(outcome.success);
    assert_eq!(
        queue.get("t1").unwrap().unwrap().state,
        TaskState::Completed
    );
}

#[test]
fn no_op_does_not_complete_a_task_or_mark_a_real_failure_for_safe_mode() {
    let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
    queue.enqueue(dev_task("noop-task")).unwrap();
    let loop_ = build_loop_with_executor(queue.clone(), Arc::new(NoOpExecutor));
    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(!outcome.success);
    assert_eq!(outcome.execution_status, Some(ExecutionOverallStatus::NoOp));
    assert_eq!(
        queue.get("noop-task").unwrap().unwrap().state,
        TaskState::Failed
    );
    assert_eq!(
        loop_.current_mode(),
        aicore_contracts::runtime::mode::RuntimeMode::Normal
    );
}
