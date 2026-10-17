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

use aicore_contracts::{
    context::workload::InteractionLevel,
    errors::ContractError,
    memory::profile::UserProfile,
    policy::{priority::PolicyPriority, resource_policy::PowerMode},
    task::preferences::{HardwareRequirement, LatencySensitivity, PowerPreference},
    traits::{memory_store::IMemoryStore, task_queue::IPreparationAuthorization},
};
use std::sync::{atomic::AtomicBool, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct RecordingExecutor {
    calls: Mutex<Vec<(ResourcePolicy, ExecutionTargets)>>,
}
impl IPolicyExecutor for RecordingExecutor {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
        self.calls
            .lock()
            .unwrap()
            .push((policy.clone(), targets.clone()));
        OkExecutor.execute(policy, targets)
    }
    fn name(&self) -> &'static str {
        "recording_executor"
    }
}

struct PreparationGuard {
    revoked: AtomicBool,
}
impl IPreparationAuthorization for PreparationGuard {
    fn targets(&self) -> ContractResult<ExecutionTargets> {
        if self.revoked.load(Ordering::SeqCst) {
            return Err(ContractError::InvalidRequest(
                "authorization revoked".into(),
            ));
        }
        Ok(ExecutionTargets {
            process_only: true,
            foreground_pids: vec![202],
            background_pids: vec![],
        })
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Catch mixing L1 identity/constraints with a dequeued application's context,
// authorization and completion. Both applications deliberately disagree.
#[test]
fn l1_task_owns_the_entire_tick_and_leaves_queue_preparation_pending() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    let hub = Arc::new(aicore_tier::TierHub::new());
    let (_, source) = aicore_tier_l1::register_l1(&hub).unwrap();
    let mut declared = TaskContract::minimal(ApplicationId::new("app.declared"));
    declared.task_id = Some("declared-a".into());
    declared.task = Some(WorkloadType::Gaming);
    declared.latency = Some(LatencySensitivity::Critical);
    declared.priority = Some(PolicyPriority::High);
    declared.power = Some(PowerPreference::Performance);
    declared.gpu_requirement = Some(HardwareRequirement::Preferred);
    declared.npu_requirement = Some(HardwareRequirement::NotNeeded);
    source.set_current(declared).unwrap();
    let mut pending = dev_task("queued-b");
    pending.contract.priority = Some(PolicyPriority::Low);
    pending.contract.power = Some(PowerPreference::PowerSave);
    pending.contract.latency = Some(LatencySensitivity::Low);
    pending.contract.gpu_requirement = Some(HardwareRequirement::NotNeeded);
    pending.contract.npu_requirement = Some(HardwareRequirement::Preferred);
    pending.contract.deadline_unix_secs = Some(now_secs() + 60);
    let guard = Arc::new(PreparationGuard {
        revoked: AtomicBool::new(false),
    });
    queue
        .enqueue_preparation(pending, 0, guard.clone())
        .unwrap();
    let memory = Arc::new(aicore_memory::InMemoryStore::new());
    memory
        .save_profile(&UserProfile {
            history_enabled: true,
            ..UserProfile::default()
        })
        .unwrap();
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone())
        .with_tier_hub(hub)
        .with_memory(memory.clone());
    let supplied = ExecutionTargets {
        process_only: true,
        foreground_pids: vec![101],
        background_pids: vec![],
    };
    let outcome = loop_.tick(&supplied);
    assert!(outcome.success);
    assert_eq!(outcome.workload_type, WorkloadType::Gaming);
    let context: aicore_contracts::context::snapshot::ContextSnapshot =
        serde_json::from_str(outcome.current_context_json.as_ref().unwrap()).unwrap();
    assert_eq!(context.interaction_level, InteractionLevel::Interactive);
    {
        let calls = executor.calls.lock().unwrap();
        let (policy, targets) = &calls[0];
        assert_eq!(targets, &supplied);
        assert_eq!(policy.task_id.as_deref(), Some("declared-a"));
        assert_eq!(policy.priority, PolicyPriority::High);
        assert_eq!(policy.power.mode, PowerMode::Performance);
        assert_eq!(policy.gpu.workload_preference.as_deref(), Some("compute"));
        assert!(!policy.npu.preferred);
    }
    let record = memory.load_records(1).unwrap().pop().unwrap();
    assert_eq!(
        record.application_id,
        Some(ApplicationId::new("app.declared"))
    );
    assert_eq!(record.workload_type, WorkloadType::Gaming);
    assert_eq!(
        queue.get("queued-b").unwrap().unwrap().state,
        TaskState::Queued
    );
    assert!(queue.preparation_targets("queued-b").unwrap().is_some());
    source.clear().unwrap();
    assert!(loop_.tick(&supplied).success);
    assert_eq!(
        queue.get("queued-b").unwrap().unwrap().state,
        TaskState::Completed
    );
    let calls = executor.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].1.foreground_pids, vec![202]);
    assert_eq!(calls[1].0.task_id.as_deref(), Some("queued-b"));
    assert_eq!(calls[1].0.priority, PolicyPriority::Low);
    assert_eq!(calls[1].0.power.mode, PowerMode::PowerSave);
    assert_eq!(calls[1].0.gpu.workload_preference, None);
    assert!(calls[1].0.npu.preferred);
    let record = memory.load_records(1).unwrap().pop().unwrap();
    assert_eq!(
        record.application_id,
        Some(ApplicationId::new("com.example.ide"))
    );
    assert_eq!(record.workload_type, WorkloadType::Development);
    assert!(queue.preparation_targets("queued-b").unwrap().is_none());
    assert_eq!(Arc::strong_count(&guard), 1);
}

// Catch disarming cleanup when a Running task's own deadline passes while
// the prepared policy is still fresh. Only the wall clock is used, no host IO.
#[test]
fn running_deadline_before_apply_closes_task_and_releases_authorization() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    let mut task = dev_task("deadline");
    let deadline = now_secs() + 2;
    task.contract.deadline_unix_secs = Some(deadline);
    let guard = Arc::new(PreparationGuard {
        revoked: AtomicBool::new(false),
    });
    queue.enqueue_preparation(task, 0, guard.clone()).unwrap();
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
    let prepared = loop_.prepare_tick(Some(Duration::from_secs(60))).unwrap();
    assert_eq!(
        queue.get("deadline").unwrap().unwrap().state,
        TaskState::Running
    );
    let deadline_time = UNIX_EPOCH + Duration::from_secs(deadline);
    if let Ok(remaining) = deadline_time.duration_since(SystemTime::now()) {
        std::thread::sleep(remaining);
    }
    let outcome = loop_.apply_prepared_tick(prepared, &ExecutionTargets::default());
    assert!(!outcome.success);
    assert!(executor.calls.lock().unwrap().is_empty());
    let task = queue.get("deadline").unwrap().unwrap();
    assert_eq!(task.state, TaskState::Failed);
    assert!(task.finished_at_unix_ms.is_some());
    assert!(queue.preparation_targets("deadline").unwrap().is_none());
    assert_eq!(Arc::strong_count(&guard), 1);
}

// Preserve the real queue's transitions/cleanup; inject only an unavailable
// status read, which must decline execution without abandoning Running work.
struct ReadFaultQueue {
    inner: InMemoryTaskQueue,
    fail_get: AtomicBool,
}
impl ITaskQueue for ReadFaultQueue {
    fn enqueue(&self, task: QueuedTask) -> ContractResult<String> {
        self.inner.enqueue(task)
    }
    fn dequeue(&self) -> ContractResult<Option<QueuedTask>> {
        self.inner.dequeue()
    }
    fn complete(&self, id: &str, message: Option<String>) -> ContractResult<()> {
        self.inner.complete(id, message)
    }
    fn fail(&self, id: &str, message: String) -> ContractResult<()> {
        self.inner.fail(id, message)
    }
    fn cancel(&self, id: &str) -> ContractResult<()> {
        self.inner.cancel(id)
    }
    fn get(&self, id: &str) -> ContractResult<Option<QueuedTask>> {
        if self.fail_get.swap(false, Ordering::SeqCst) {
            Err(ContractError::AdapterUnavailable(
                "status read unavailable".into(),
            ))
        } else {
            self.inner.get(id)
        }
    }
    fn snapshot(&self) -> ContractResult<Vec<QueuedTask>> {
        self.inner.snapshot()
    }
    fn queued_count(&self) -> ContractResult<usize> {
        self.inner.queued_count()
    }
    fn queue_name(&self) -> &'static str {
        "read_fault_queue"
    }
}

#[test]
fn status_read_failure_before_apply_fails_task_without_execution() {
    let queue = Arc::new(ReadFaultQueue {
        inner: InMemoryTaskQueue::new(),
        fail_get: AtomicBool::new(false),
    });
    queue.enqueue(dev_task("read-error")).unwrap();
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
    let prepared = loop_.prepare_tick(None).unwrap();
    queue.fail_get.store(true, Ordering::SeqCst);
    assert!(
        !loop_
            .apply_prepared_tick(prepared, &ExecutionTargets::default())
            .success
    );
    assert_eq!(
        queue.get("read-error").unwrap().unwrap().state,
        TaskState::Failed
    );
    assert!(executor.calls.lock().unwrap().is_empty());
}

#[test]
fn discarded_or_stale_plan_closes_running_task_and_releases_authorization() {
    for stale in [false, true] {
        let queue = Arc::new(InMemoryTaskQueue::new());
        let mut task = dev_task("discard");
        task.contract.deadline_unix_secs = Some(now_secs() + 60);
        let guard = Arc::new(PreparationGuard {
            revoked: AtomicBool::new(false),
        });
        queue.enqueue_preparation(task, 0, guard.clone()).unwrap();
        let executor = Arc::new(RecordingExecutor::default());
        let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
        let prepared = loop_
            .prepare_tick(if stale { Some(Duration::ZERO) } else { None })
            .unwrap();
        if stale {
            assert!(
                !loop_
                    .apply_prepared_tick(prepared, &ExecutionTargets::default())
                    .success
            );
        } else {
            drop(prepared);
        }
        assert_eq!(
            queue.get("discard").unwrap().unwrap().state,
            TaskState::Failed
        );
        assert!(queue.preparation_targets("discard").unwrap().is_none());
        assert_eq!(Arc::strong_count(&guard), 1);
        assert!(executor.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn cancellation_before_apply_is_preserved_without_execution() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    queue.enqueue(dev_task("cancelled")).unwrap();
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
    let prepared = loop_.prepare_tick(None).unwrap();
    queue.cancel("cancelled").unwrap();
    let cancelled = queue.get("cancelled").unwrap().unwrap();
    assert!(
        !loop_
            .apply_prepared_tick(prepared, &ExecutionTargets::default())
            .success
    );
    assert_eq!(queue.get("cancelled").unwrap().unwrap(), cancelled);
    assert!(executor.calls.lock().unwrap().is_empty());
}

#[test]
fn dropping_plan_preserves_each_existing_terminal_state() {
    for terminal in [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
    ] {
        let queue = Arc::new(InMemoryTaskQueue::new());
        queue.enqueue(dev_task("terminal")).unwrap();
        let loop_ = build_loop(queue.clone());
        let prepared = loop_.prepare_tick(None).unwrap();
        match terminal {
            TaskState::Completed => queue
                .complete("terminal", Some("completed elsewhere".into()))
                .unwrap(),
            TaskState::Failed => queue.fail("terminal", "failed elsewhere".into()).unwrap(),
            TaskState::Cancelled => queue.cancel("terminal").unwrap(),
            _ => unreachable!(),
        }
        let before = queue.get("terminal").unwrap().unwrap();
        drop(prepared);
        assert_eq!(queue.get("terminal").unwrap().unwrap(), before);
    }
}

#[test]
fn authorization_revoked_after_prepare_fails_without_execution_and_releases_guard() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    let mut task = dev_task("revoked");
    task.contract.deadline_unix_secs = Some(now_secs() + 60);
    let guard = Arc::new(PreparationGuard {
        revoked: AtomicBool::new(false),
    });
    queue.enqueue_preparation(task, 0, guard.clone()).unwrap();
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
    let prepared = loop_.prepare_tick(None).unwrap();
    guard.revoked.store(true, Ordering::SeqCst);
    assert!(
        !loop_
            .apply_prepared_tick(prepared, &ExecutionTargets::default())
            .success
    );
    assert_eq!(
        queue.get("revoked").unwrap().unwrap().state,
        TaskState::Failed
    );
    assert!(executor.calls.lock().unwrap().is_empty());
    assert_eq!(Arc::strong_count(&guard), 1);
}

struct FailedExecutor;
impl IPolicyExecutor for FailedExecutor {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
        let mut result = OkExecutor.execute(policy, targets);
        result.overall_status = ExecutionOverallStatus::AllFailed;
        result.outcomes[0].status = ExecutionStatus::Failed;
        result
    }
    fn name(&self) -> &'static str {
        "failed_executor"
    }
}

// Preserve the queue's real lifecycle and authorization cleanup, injecting
// only a single failed terminal write at its external boundary.
struct FinishFaultQueue {
    inner: InMemoryTaskQueue,
    fail_complete: AtomicBool,
    fail_fail: AtomicBool,
    cancel_on_complete_error: AtomicBool,
}
impl FinishFaultQueue {
    fn new(complete_error: bool, fail_error: bool) -> Self {
        Self {
            inner: InMemoryTaskQueue::new(),
            fail_complete: AtomicBool::new(complete_error),
            fail_fail: AtomicBool::new(fail_error),
            cancel_on_complete_error: AtomicBool::new(false),
        }
    }
}
impl ITaskQueue for FinishFaultQueue {
    fn enqueue(&self, task: QueuedTask) -> ContractResult<String> {
        self.inner.enqueue(task)
    }
    fn enqueue_preparation(
        &self,
        task: QueuedTask,
        ready: u64,
        guard: Arc<dyn IPreparationAuthorization>,
    ) -> ContractResult<String> {
        self.inner.enqueue_preparation(task, ready, guard)
    }
    fn preparation_targets(&self, id: &str) -> ContractResult<Option<ExecutionTargets>> {
        self.inner.preparation_targets(id)
    }
    fn dequeue(&self) -> ContractResult<Option<QueuedTask>> {
        self.inner.dequeue()
    }
    fn complete(&self, id: &str, message: Option<String>) -> ContractResult<()> {
        if self.fail_complete.swap(false, Ordering::SeqCst) {
            if self.cancel_on_complete_error.load(Ordering::SeqCst) {
                self.inner.cancel(id)?;
            }
            return Err(ContractError::AdapterUnavailable(
                "one-shot completion write failure".into(),
            ));
        }
        self.inner.complete(id, message)
    }
    fn fail(&self, id: &str, message: String) -> ContractResult<()> {
        if self.fail_fail.swap(false, Ordering::SeqCst) {
            return Err(ContractError::AdapterUnavailable(
                "one-shot failure write failure".into(),
            ));
        }
        self.inner.fail(id, message)
    }
    fn cancel(&self, id: &str) -> ContractResult<()> {
        self.inner.cancel(id)
    }
    fn get(&self, id: &str) -> ContractResult<Option<QueuedTask>> {
        self.inner.get(id)
    }
    fn snapshot(&self) -> ContractResult<Vec<QueuedTask>> {
        self.inner.snapshot()
    }
    fn queued_count(&self) -> ContractResult<usize> {
        self.inner.queued_count()
    }
    fn queue_name(&self) -> &'static str {
        "finish_fault_queue"
    }
}

fn enqueue_fault_preparation(queue: &FinishFaultQueue) -> Arc<PreparationGuard> {
    let mut task = dev_task("write-error");
    task.contract.deadline_unix_secs = Some(now_secs() + 60);
    let guard = Arc::new(PreparationGuard {
        revoked: AtomicBool::new(false),
    });
    queue.enqueue_preparation(task, 0, guard.clone()).unwrap();
    guard
}

fn assert_failed_and_released(queue: &FinishFaultQueue, guard: &Arc<PreparationGuard>) {
    let task = queue.get("write-error").unwrap().unwrap();
    assert_eq!(task.state, TaskState::Failed);
    assert!(task.finished_at_unix_ms.is_some());
    assert!(queue.preparation_targets("write-error").unwrap().is_none());
    assert_eq!(Arc::strong_count(guard), 1);
}

#[test]
fn one_shot_complete_error_does_not_abandon_running_task() {
    let queue = Arc::new(FinishFaultQueue::new(true, false));
    let guard = enqueue_fault_preparation(&queue);
    let loop_ = build_loop(queue.clone());
    assert!(loop_.tick(&ExecutionTargets::default()).success);
    assert_failed_and_released(&queue, &guard);
}

#[test]
fn one_shot_execution_failure_write_error_does_not_abandon_running_task() {
    let queue = Arc::new(FinishFaultQueue::new(false, true));
    let guard = enqueue_fault_preparation(&queue);
    let loop_ = build_loop_with_executor(queue.clone(), Arc::new(FailedExecutor));
    assert!(!loop_.tick(&ExecutionTargets::default()).success);
    assert_failed_and_released(&queue, &guard);
}

#[test]
fn one_shot_validation_failure_write_error_does_not_abandon_running_task() {
    use aicore_contracts::traits::policy_validator::{IPolicyValidator, ValidationResult};
    struct Reject;
    impl IPolicyValidator for Reject {
        fn validate(&self, _: &ResourcePolicy) -> ValidationResult {
            ValidationResult {
                accepted: false,
                issues: vec![],
            }
        }
        fn name(&self) -> &'static str {
            "reject"
        }
    }
    let queue = Arc::new(FinishFaultQueue::new(false, true));
    let guard = enqueue_fault_preparation(&queue);
    let executor = Arc::new(RecordingExecutor::default());
    let engine = Arc::new(RuleBasedPolicyEngine::new());
    let loop_ = AdaptiveLoop::new(
        Arc::new(MockTelemetry::new()),
        Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
        engine.clone(),
        engine,
        Arc::new(Reject),
        executor.clone(),
        AdaptiveLoopConfig::default(),
    )
    .with_task_queue(queue.clone());
    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(!outcome.validation_accepted);
    assert!(executor.calls.lock().unwrap().is_empty());
    assert_failed_and_released(&queue, &guard);
}

#[test]
fn one_shot_dropped_plan_failure_write_error_does_not_abandon_running_task() {
    let queue = Arc::new(FinishFaultQueue::new(false, true));
    let guard = enqueue_fault_preparation(&queue);
    let executor = Arc::new(RecordingExecutor::default());
    let loop_ = build_loop_with_executor(queue.clone(), executor.clone());
    drop(loop_.prepare_tick(None).unwrap());
    assert!(executor.calls.lock().unwrap().is_empty());
    assert_failed_and_released(&queue, &guard);
}

#[test]
fn complete_error_after_concurrent_cancel_preserves_cancelled_state() {
    let queue = Arc::new(FinishFaultQueue::new(true, false));
    queue.cancel_on_complete_error.store(true, Ordering::SeqCst);
    let guard = enqueue_fault_preparation(&queue);
    let loop_ = build_loop(queue.clone());
    assert!(loop_.tick(&ExecutionTargets::default()).success);
    let task = queue.get("write-error").unwrap().unwrap();
    assert_eq!(task.state, TaskState::Cancelled);
    assert_eq!(task.result_message, None);
    assert!(queue.preparation_targets("write-error").unwrap().is_none());
    assert_eq!(Arc::strong_count(&guard), 1);
}

#[test]
fn failed_execution_closes_task_and_releases_preparation_authorization() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    let mut task = dev_task("failed");
    task.contract.deadline_unix_secs = Some(now_secs() + 60);
    let guard = Arc::new(PreparationGuard {
        revoked: AtomicBool::new(false),
    });
    queue.enqueue_preparation(task, 0, guard.clone()).unwrap();
    let loop_ = build_loop_with_executor(queue.clone(), Arc::new(FailedExecutor));
    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(!outcome.success);
    assert_eq!(
        outcome.execution_status,
        Some(ExecutionOverallStatus::AllFailed)
    );
    assert_eq!(
        queue.get("failed").unwrap().unwrap().state,
        TaskState::Failed
    );
    assert!(queue.preparation_targets("failed").unwrap().is_none());
    assert_eq!(Arc::strong_count(&guard), 1);
}

struct ConcurrentExecutor {
    entered: Arc<std::sync::Barrier>,
    resume: Arc<std::sync::Barrier>,
    fail: bool,
}
impl IPolicyExecutor for ConcurrentExecutor {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
        self.entered.wait();
        self.resume.wait();
        if self.fail {
            FailedExecutor.execute(policy, targets)
        } else {
            OkExecutor.execute(policy, targets)
        }
    }
    fn name(&self) -> &'static str {
        "concurrent_executor"
    }
}

#[test]
fn concurrent_cancellation_during_execution_is_not_overwritten_by_finish() {
    for fail in [false, true] {
        let queue = Arc::new(InMemoryTaskQueue::new());
        queue.enqueue(dev_task("concurrent")).unwrap();
        let entered = Arc::new(std::sync::Barrier::new(2));
        let resume = Arc::new(std::sync::Barrier::new(2));
        let loop_ = Arc::new(build_loop_with_executor(
            queue.clone(),
            Arc::new(ConcurrentExecutor {
                entered: entered.clone(),
                resume: resume.clone(),
                fail,
            }),
        ));
        let prepared = loop_.prepare_tick(None).unwrap();
        let worker = std::thread::spawn(move || {
            loop_.apply_prepared_tick(prepared, &ExecutionTargets::default())
        });
        entered.wait();
        queue.cancel("concurrent").unwrap();
        let cancelled = queue.get("concurrent").unwrap().unwrap();
        resume.wait();
        assert_eq!(worker.join().unwrap().success, !fail);
        assert_eq!(queue.get("concurrent").unwrap().unwrap(), cancelled);
    }
}

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
