use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use aicore_contracts::{
    execution::{result::ExecutionResult, targets::ExecutionTargets},
    memory::record::HistoricalRecord,
    policy::resource_policy::ResourcePolicy,
    runtime::tick::TickOutcome,
    traits::policy_validator::IPolicyValidator,
};

use crate::adaptive_loop::{AdaptiveLoop, PreparedTick};

/// An immutable proposal from the slow side. The expiry is checked again at
/// the point of execution, so a late inference can never actuate the host.
#[derive(Clone)]
pub struct PreparedPolicy {
    pub policy: Arc<ResourcePolicy>,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone)]
struct PreparedEnvelope {
    generation: u64,
    proposal: PreparedPolicy,
}

type PreparationJob = Box<dyn FnOnce() -> Option<PreparedPolicy> + Send>;
type PreparationRequest = (u64, PreparationJob);

/// One in-flight slow job and one queued job. Cancellation is generation
/// based: a completed job from an earlier generation is discarded.
pub struct SlowPolicyWorker {
    requests: mpsc::SyncSender<PreparationRequest>,
    results: Arc<Mutex<Option<PreparedEnvelope>>>,
    generation: Arc<AtomicU64>,
}

impl SlowPolicyWorker {
    pub fn new() -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<PreparationRequest>(1);
        let results = Arc::new(Mutex::new(None));
        let worker_results = results.clone();
        std::thread::Builder::new()
            .name("aicore-slow-policy".into())
            .spawn(move || {
                while let Ok((generation, job)) = receiver.recv() {
                    let proposal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                        .ok()
                        .flatten();
                    if let Some(proposal) = proposal {
                        if let Ok(mut latest) = worker_results.lock() {
                            *latest = Some(PreparedEnvelope {
                                generation,
                                proposal,
                            });
                        }
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Returns false immediately when the bounded worker is busy.
    pub fn submit(&self, job: impl FnOnce() -> Option<PreparedPolicy> + Send + 'static) -> bool {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.requests.try_send((generation, Box::new(job))).is_ok()
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn latest(&self) -> Option<PreparedEnvelope> {
        self.results
            .lock()
            .ok()?
            .take()
            .filter(|result| result.generation == self.generation.load(Ordering::SeqCst))
    }
}

pub enum FastPolicyOutcome {
    Executed(ExecutionResult),
    Rejected,
    Pending,
    /// Cancellation requested; the executor has NOT yet exited.
    Cancelling,
    TimedOut,
    Cancelled,
    Failed(String),
}

#[derive(Clone)]
pub struct ExecutionCancellation {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    deadline: std::time::Instant,
    generation: Arc<AtomicU64>,
    expected_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStop {
    Cancelled,
    TimedOut,
}

impl ExecutionCancellation {
    pub fn check(&self) -> Result<(), ExecutionStop> {
        if std::time::Instant::now() >= self.deadline {
            Err(ExecutionStop::TimedOut)
        } else if self.cancelled.load(Ordering::Acquire)
            || self.generation.load(Ordering::Acquire) != self.expected_generation
        {
            Err(ExecutionStop::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Trusted, cooperative execution contract. Implementations must check the
/// token around bounded operations, release resources/restore on cancellation,
/// and return only after cleanup. Legacy synchronous executors are deliberately
/// not adapted: a timeout cannot safely kill an arbitrary Rust thread.
pub trait CancellablePolicyExecutor: Send + Sync + 'static {
    fn execute_cancellable(
        &self,
        policy: &ResourcePolicy,
        targets: &ExecutionTargets,
        cancellation: &ExecutionCancellation,
    ) -> Result<ExecutionResult, ExecutionStop>;
}

// Even a contract-violating executor cannot create unbounded detached workers
// by repeatedly constructing/dropping consumers. Its permit stays in the thread.
static EXECUTION_WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const MAX_EXECUTION_WORKERS: usize = 32;
struct ExecutionPermit;
impl Drop for ExecutionPermit {
    fn drop(&mut self) {
        EXECUTION_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
struct ActiveExecution {
    worker: std::thread::JoinHandle<FastPolicyOutcome>,
    cancellation: ExecutionCancellation,
}

/// Consumes only precomputed policies. It does no telemetry, history, AI or
/// runtime binding and never waits for the slow worker. The fallback is a
/// caller-supplied rule policy, not a fabricated successful execution.
pub struct FastPolicyConsumer {
    worker: Arc<SlowPolicyWorker>,
    current: Mutex<Option<PreparedEnvelope>>,
    fallback: Arc<ResourcePolicy>,
    validator: Arc<dyn IPolicyValidator>,
    executor: Arc<dyn CancellablePolicyExecutor>,
    active: Mutex<Option<ActiveExecution>>,
    timeout: std::time::Duration,
}

impl FastPolicyConsumer {
    pub fn new(
        worker: Arc<SlowPolicyWorker>,
        fallback: Arc<ResourcePolicy>,
        validator: Arc<dyn IPolicyValidator>,
        executor: Arc<dyn CancellablePolicyExecutor>,
    ) -> Self {
        Self {
            worker,
            current: Mutex::new(None),
            fallback,
            validator,
            executor,
            active: Mutex::new(None),
            timeout: std::time::Duration::from_millis(100),
        }
    }

    pub fn with_execution_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout.min(std::time::Duration::from_secs(300));
        self
    }

    pub fn cancel_execution(&self) {
        if let Ok(active) = self.active.lock() {
            if let Some(active) = &*active {
                active.cancellation.cancelled.store(true, Ordering::Release);
            }
        }
    }

    pub fn tick(&self, now_unix_ms: u64, targets: &ExecutionTargets) -> FastPolicyOutcome {
        let mut active = match self.active.try_lock() {
            Ok(active) => active,
            Err(std::sync::TryLockError::WouldBlock) => return FastPolicyOutcome::Pending,
            Err(_) => return FastPolicyOutcome::Failed("execution state poisoned".into()),
        };
        if let Some(task) = &*active {
            if !task.worker.is_finished() {
                return if task.cancellation.check().is_err() {
                    FastPolicyOutcome::Cancelling
                } else {
                    FastPolicyOutcome::Pending
                };
            }
            let task = active.take().unwrap();
            return task
                .worker
                .join()
                .unwrap_or_else(|_| FastPolicyOutcome::Failed("executor panicked".into()));
        }
        if let Some(proposal) = self.worker.latest() {
            if let Ok(mut current) = self.current.lock() {
                *current = Some(proposal);
            }
        }
        let envelope = self
            .current
            .lock()
            .ok()
            .and_then(|current| current.clone())
            .filter(|envelope| {
                envelope.generation == self.worker.generation.load(Ordering::SeqCst)
                    && envelope.proposal.expires_at_unix_ms > now_unix_ms
            });
        let (policy, generation, max_age) = match envelope {
            Some(envelope) => (
                envelope.proposal.policy,
                envelope.generation,
                std::time::Duration::from_millis(
                    envelope.proposal.expires_at_unix_ms - now_unix_ms,
                )
                .min(self.timeout),
            ),
            None => (
                self.fallback.clone(),
                self.worker.generation.load(Ordering::Acquire),
                self.timeout,
            ),
        };
        if EXECUTION_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_EXECUTION_WORKERS).then_some(n + 1)
            })
            .is_err()
        {
            return FastPolicyOutcome::Failed("execution worker capacity reached".into());
        }
        let permit = ExecutionPermit;
        let cancellation = ExecutionCancellation {
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            deadline: std::time::Instant::now() + max_age,
            generation: self.worker.generation.clone(),
            expected_generation: generation,
        };
        let token = cancellation.clone();
        let executor = self.executor.clone();
        let validator = self.validator.clone();
        let targets = targets.clone();
        match std::thread::Builder::new()
            .name("aicore-policy-execution".into())
            .spawn(move || {
                let _permit = permit;
                if let Err(stop) = token.check() {
                    return stop_outcome(stop);
                }
                if !validator.validate(&policy).accepted {
                    return FastPolicyOutcome::Rejected;
                }
                if let Err(stop) = token.check() {
                    return stop_outcome(stop);
                }
                match executor.execute_cancellable(&policy, &targets, &token) {
                    Ok(result) => FastPolicyOutcome::Executed(result),
                    Err(stop) => stop_outcome(stop),
                }
            }) {
            Ok(worker) => {
                *active = Some(ActiveExecution {
                    worker,
                    cancellation,
                });
                FastPolicyOutcome::Pending
            },
            Err(error) => {
                FastPolicyOutcome::Failed(format!("execution worker unavailable: {error}"))
            },
        }
    }
}

fn stop_outcome(stop: ExecutionStop) -> FastPolicyOutcome {
    match stop {
        ExecutionStop::Cancelled => FastPolicyOutcome::Cancelled,
        ExecutionStop::TimedOut => FastPolicyOutcome::TimedOut,
    }
}

impl Drop for FastPolicyConsumer {
    fn drop(&mut self) {
        let active = self.active.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = active.take() {
            task.cancellation.cancelled.store(true, Ordering::Release);
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
            while !task.worker.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if task.worker.is_finished() {
                let _ = task.worker.join();
            } else {
                tracing::error!(
                    "cooperative executor failed to exit; worker capacity remains reserved"
                );
            }
        }
    }
}

/// Daemon-facing split of the existing AdaptiveLoop. A single bounded worker
/// performs slow preparation. The scheduler thread consumes only ready plans
/// or records a neutral NoOp. Model binding is safe on the slow worker.
pub struct ModelTickDriver {
    loop_: Arc<AdaptiveLoop>,
    requests: mpsc::SyncSender<()>,
    results: mpsc::Receiver<Result<PreparedTick, Box<TickOutcome>>>,
    apply_requests: mpsc::SyncSender<PreparedTick>,
    applied: mpsc::Receiver<TickOutcome>,
    applying: std::sync::atomic::AtomicBool,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    published_ticks: AtomicU64,
}

impl ModelTickDriver {
    pub fn new(
        loop_: Arc<AdaptiveLoop>,
        targets: ExecutionTargets,
        max_age: std::time::Duration,
    ) -> std::io::Result<Self> {
        Self::new_inner(loop_, targets, max_age, false)
    }

    fn new_inner(
        loop_: Arc<AdaptiveLoop>,
        targets: ExecutionTargets,
        max_age: std::time::Duration,
        platform: bool,
    ) -> std::io::Result<Self> {
        if max_age.is_zero() || loop_.has_model_only_effects() == platform {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "split driver does not match its executor and binder",
            ));
        }
        let (requests, received) = mpsc::sync_channel(1);
        let (delivered, results) = mpsc::sync_channel(1);
        let worker_loop = loop_.clone();
        std::thread::Builder::new()
            .name(if platform {
                "aicore-platform-slow-loop".into()
            } else {
                "aicore-model-slow-loop".into()
            })
            .spawn(move || {
                while received.recv().is_ok() {
                    // A full result channel drops the newly prepared plan. Its task
                    // guard fails dequeued work instead of losing state.
                    let start = std::time::Instant::now();
                    let plan = if platform {
                        worker_loop.prepare_platform_tick(max_age)
                    } else {
                        worker_loop.prepare_tick(Some(max_age))
                    };
                    worker_loop
                        .metrics_collector()
                        .record_slow_prepare(start.elapsed());
                    let _ = delivered.try_send(plan);
                }
            })?;
        let history_sender = if let Some(memory) = loop_.memory_store() {
            let (sender, records) = mpsc::sync_channel::<HistoricalRecord>(8);
            std::thread::Builder::new()
                .name("aicore-split-history".into())
                .spawn(move || {
                    while let Ok(record) = records.recv() {
                        if AdaptiveLoop::may_record_history(&memory) {
                            if let Err(error) = memory.append_record(&record) {
                                tracing::warn!(component="model_history", error=%error, "history write failed");
                            }
                        }
                    }
                })?;
            Some(sender)
        } else {
            None
        };
        let (apply_requests, received_apply) = mpsc::sync_channel::<PreparedTick>(1);
        let (completed, applied) = mpsc::sync_channel(1);
        let apply_loop = loop_.clone();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name("aicore-apply-loop".into())
            .spawn(move || {
                while let Ok(plan) = received_apply.recv() {
                    if worker_cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    // Freshness, mode and task authorization are still checked
                    // immediately before apply, on this single serialized worker.
                    let outcome = apply_loop.apply_prepared_tick_with_history_sender(
                        plan,
                        &targets,
                        history_sender.as_ref(),
                    );
                    if completed.send(outcome).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            loop_,
            requests,
            results,
            apply_requests,
            applied,
            applying: std::sync::atomic::AtomicBool::new(false),
            cancelled,
            published_ticks: AtomicU64::new(0),
        })
    }

    pub fn tick(&self) -> TickOutcome {
        let start = std::time::Instant::now();
        let completed = self.applied.try_recv();
        let mut outcome = if matches!(completed, Err(mpsc::TryRecvError::Disconnected)) {
            self.loop_
                .worker_failed_tick("policy application worker disconnected")
        } else if let Ok(outcome) = completed {
            self.applying.store(false, Ordering::SeqCst);
            outcome
        } else if self.applying.load(Ordering::SeqCst) {
            self.loop_
                .neutral_tick("serialized policy application pending")
        } else {
            match self.results.try_recv() {
                Ok(Ok(plan)) => {
                    if self.apply_requests.try_send(plan).is_ok() {
                        self.applying.store(true, Ordering::SeqCst);
                        self.loop_
                            .neutral_tick("serialized policy application pending")
                    } else {
                        self.loop_
                            .worker_failed_tick("policy application worker unavailable")
                    }
                },
                Ok(Err(outcome)) => *outcome,
                Err(mpsc::TryRecvError::Disconnected) => self
                    .loop_
                    .worker_failed_tick("policy preparation worker disconnected"),
                Err(mpsc::TryRecvError::Empty) => {
                    let _ = self.requests.try_send(());
                    self.loop_.neutral_tick("slow policy preparation pending")
                },
            }
        };
        // Slow preparations can complete after a newer neutral fast tick.
        // External daemon status/history IDs follow publication order.
        outcome.tick_id = self.published_ticks.fetch_add(1, Ordering::SeqCst) + 1;
        self.loop_
            .metrics_collector()
            .record_fast_control(start.elapsed());
        outcome
    }
}

impl Drop for ModelTickDriver {
    fn drop(&mut self) {
        // Rust cannot safely kill a synchronous provider in flight. Prevent
        // queued applications from starting; never wait on it on the fast side.
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Platform split driver. Preparation and serialized application are bounded
/// handoffs; neither synchronous binding, setters nor callbacks run on polling.
/// In-flight OS calls are not forcibly cancelled or certified as hard real time.
pub struct PlatformTickDriver {
    inner: ModelTickDriver,
}

impl PlatformTickDriver {
    pub fn new(
        loop_: Arc<AdaptiveLoop>,
        targets: ExecutionTargets,
        max_age: std::time::Duration,
    ) -> std::io::Result<Self> {
        Ok(Self {
            inner: ModelTickDriver::new_inner(loop_, targets, max_age, true)?,
        })
    }

    pub fn tick(&self) -> TickOutcome {
        self.inner.tick()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::traits::policy_executor::IPolicyExecutor;
    use std::time::{Duration, Instant};

    use aicore_contracts::{
        ai::runtime::ModelHandle,
        errors::ContractResult,
        execution::status::ExecutionOverallStatus,
        runtime::{
            config::AdaptiveLoopConfig,
            mode::RuntimeMode,
            route::{RuntimeRoute, RuntimeRouteSource},
        },
        telemetry::snapshot::ComputerSnapshot,
        traits::{
            policy_validator::ValidationResult,
            runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
            runtime_router::IRuntimeRouter,
            telemetry_provider::ITelemetryProvider,
        },
    };
    use aicore_core::{
        context::{ContextEngine, RuleBasedClassifier},
        model::{ModelEnvironment, ModelPolicyExecutor, ModelTargetProvider},
        policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
    };

    struct DelayedTelemetry(Duration);
    impl ITelemetryProvider for DelayedTelemetry {
        fn collect(&self) -> ContractResult<ComputerSnapshot> {
            std::thread::sleep(self.0);
            Ok(ComputerSnapshot::default())
        }
        fn platform_name(&self) -> &'static str {
            "model-test"
        }
        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }
    }

    fn model_loop(delay: Duration) -> (Arc<AdaptiveLoop>, Arc<ModelEnvironment>) {
        let environment = ModelEnvironment::new();
        let rule = Arc::new(RuleBasedPolicyEngine::new());
        let loop_ = AdaptiveLoop::new(
            Arc::new(DelayedTelemetry(delay)),
            Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
            rule.clone(),
            rule,
            Arc::new(DefaultPolicyValidator::new()),
            Arc::new(ModelPolicyExecutor::new(environment.clone())),
            AdaptiveLoopConfig::default(),
        )
        .with_runtime_binder(Arc::new(crate::ModelRuntimeBinder::new()));
        (Arc::new(loop_), environment)
    }

    struct Accept;
    impl IPolicyValidator for Accept {
        fn validate(&self, _: &ResourcePolicy) -> ValidationResult {
            ValidationResult::accept()
        }
        fn name(&self) -> &'static str {
            "accept_mock"
        }
    }
    struct ModelExecutor;
    impl CancellablePolicyExecutor for ModelExecutor {
        fn execute_cancellable(
            &self,
            policy: &ResourcePolicy,
            targets: &ExecutionTargets,
            token: &ExecutionCancellation,
        ) -> Result<ExecutionResult, ExecutionStop> {
            token.check()?;
            Ok(self.execute(policy, targets))
        }
    }
    impl IPolicyExecutor for ModelExecutor {
        fn execute(&self, policy: &ResourcePolicy, _: &ExecutionTargets) -> ExecutionResult {
            ExecutionResult {
                policy_id: policy.policy_id.clone(),
                overall_status: ExecutionOverallStatus::NoOp,
                started_at_unix_ms: 0,
                finished_at_unix_ms: 0,
                outcomes: vec![],
            }
        }
        fn name(&self) -> &'static str {
            "model_mock"
        }
    }

    struct CountBinder(Arc<AtomicU64>);
    struct SlowApply(u8);
    impl CancellablePolicyExecutor for SlowApply {
        fn execute_cancellable(
            &self,
            policy: &ResourcePolicy,
            targets: &ExecutionTargets,
            token: &ExecutionCancellation,
        ) -> Result<ExecutionResult, ExecutionStop> {
            let deadline = Instant::now() + Duration::from_millis(250);
            while Instant::now() < deadline {
                token.check()?;
                std::thread::sleep(Duration::from_millis(1));
            }
            token.check()?;
            Ok(ModelExecutor.execute(policy, targets))
        }
    }
    impl IPolicyExecutor for SlowApply {
        fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
            if self.0 == 1 {
                std::thread::sleep(Duration::from_millis(250));
            }
            ModelExecutor.execute(policy, targets)
        }
        fn name(&self) -> &'static str {
            "slow_test_executor"
        }
    }
    impl IRuntimeBinder for SlowApply {
        fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult> {
            assert_ne!(self.0, 3, "binder panic fault");
            if self.0 == 0 {
                std::thread::sleep(Duration::from_millis(250));
            }
            CountBinder(Arc::new(AtomicU64::new(0))).bind_and_execute(request)
        }
        fn name(&self) -> &'static str {
            "slow_test_binder"
        }
        fn loaded_models(&self) -> Vec<ModelHandle> {
            Vec::new()
        }
    }
    impl aicore_core::event_bus::EventSubscriber for SlowApply {
        fn handle(&self, event: &aicore_core::event_bus::Event) {
            if self.0 == 2 && event.name() == "ExecutionCompleted" {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        fn name(&self) -> &'static str {
            "slow_test_subscriber"
        }
    }
    fn assert_slow_apply_does_not_block_polling(case: u8) {
        let rule = Arc::new(RuleBasedPolicyEngine::new());
        let bus = Arc::new(aicore_core::event_bus::EventBus::new());
        bus.subscribe(Arc::new(SlowApply(case)));
        let loop_ = Arc::new(
            AdaptiveLoop::new(
                Arc::new(DelayedTelemetry(Duration::ZERO)),
                Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
                rule.clone(),
                rule,
                Arc::new(Accept),
                Arc::new(SlowApply(case)),
                AdaptiveLoopConfig::default(),
            )
            .with_runtime_router(Arc::new(NativeRouter))
            .with_runtime_binder(Arc::new(SlowApply(case)))
            .with_event_bus(bus),
        );
        let driver =
            PlatformTickDriver::new(loop_, ExecutionTargets::default(), Duration::from_secs(2))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let start = Instant::now();
            let outcome = driver.tick();
            assert!(
                start.elapsed() < Duration::from_millis(100),
                "slow operation blocked fast poll, case {case}"
            );
            if case == 3 && outcome.execution_status == Some(ExecutionOverallStatus::AllFailed) {
                return;
            }
            if outcome.validation_accepted {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("slow operation never completed, case {case}");
    }
    #[test]
    fn p1_slow_binder_does_not_block_fast_poll() {
        assert_slow_apply_does_not_block_polling(0);
    }
    #[test]
    fn p1_slow_setter_does_not_block_fast_poll() {
        assert_slow_apply_does_not_block_polling(1);
    }
    #[test]
    fn p1_slow_execution_event_does_not_block_fast_poll() {
        assert_slow_apply_does_not_block_polling(2);
    }
    #[test]
    fn p1_panicked_binder_is_failure_instead_of_pending_forever() {
        assert_slow_apply_does_not_block_polling(3);
    }
    struct NativeRouter;
    impl IRuntimeRouter for NativeRouter {
        fn decide(
            &self,
            _: &ComputerSnapshot,
            context: &aicore_contracts::context::snapshot::ContextSnapshot,
        ) -> ContractResult<RuntimeRoute> {
            Ok(RuntimeRoute::native(
                context.workload_type,
                "mock native route",
            ))
        }

        fn name(&self) -> &'static str {
            "native_mock"
        }
    }

    impl IRuntimeBinder for CountBinder {
        fn bind_and_execute(&self, _: &BindingRequest) -> ContractResult<BindingResult> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(BindingResult {
                route_id: "mock-route".into(),
                source: RuntimeRouteSource::Native,
                executed: false,
                inference: None,
                reason: "mock binding did not execute".into(),
                latency_ms: 0,
            })
        }

        fn name(&self) -> &'static str {
            "count_binder"
        }

        fn loaded_models(&self) -> Vec<ModelHandle> {
            Vec::new()
        }
    }

    fn platform_mock_loop(delay: Duration, calls: Arc<AtomicU64>) -> Arc<AdaptiveLoop> {
        let rule = Arc::new(RuleBasedPolicyEngine::new());
        Arc::new(
            AdaptiveLoop::new(
                Arc::new(DelayedTelemetry(delay)),
                Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
                rule.clone(),
                rule,
                Arc::new(DefaultPolicyValidator::new()),
                Arc::new(ModelExecutor),
                AdaptiveLoopConfig::default(),
            )
            .with_runtime_router(Arc::new(NativeRouter))
            .with_runtime_binder(Arc::new(CountBinder(calls))),
        )
    }

    fn consumer(worker: Arc<SlowPolicyWorker>) -> FastPolicyConsumer {
        FastPolicyConsumer::new(
            worker,
            Arc::new(ResourcePolicy {
                policy_id: "fallback".into(),
                ..Default::default()
            }),
            Arc::new(Accept),
            Arc::new(ModelExecutor),
        )
    }

    #[test]
    fn p1_legacy_fast_tick_does_not_wait_for_executor() {
        let fast = FastPolicyConsumer::new(
            Arc::new(SlowPolicyWorker::new().unwrap()),
            Arc::new(ResourcePolicy::default()),
            Arc::new(Accept),
            Arc::new(SlowApply(1)),
        );
        let start = Instant::now();
        let _ = fast.tick(10, &ExecutionTargets::default());
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "synchronous executor blocked fast tick"
        );
    }

    struct CooperativeProbe {
        calls: Arc<AtomicU64>,
        exits: Arc<AtomicU64>,
        panic: bool,
    }
    impl CancellablePolicyExecutor for CooperativeProbe {
        fn execute_cancellable(
            &self,
            _: &ResourcePolicy,
            _: &ExecutionTargets,
            token: &ExecutionCancellation,
        ) -> Result<ExecutionResult, ExecutionStop> {
            struct Exit(Arc<AtomicU64>);
            impl Drop for Exit {
                fn drop(&mut self) {
                    self.0.fetch_add(1, Ordering::SeqCst);
                }
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _exit = Exit(self.exits.clone());
            assert!(!self.panic, "executor fault");
            loop {
                token.check()?;
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    fn probe_consumer(panic: bool) -> (Arc<FastPolicyConsumer>, Arc<AtomicU64>, Arc<AtomicU64>) {
        let calls = Arc::new(AtomicU64::new(0));
        let exits = Arc::new(AtomicU64::new(0));
        let fast = FastPolicyConsumer::new(
            Arc::new(SlowPolicyWorker::new().unwrap()),
            Arc::new(ResourcePolicy::default()),
            Arc::new(Accept),
            Arc::new(CooperativeProbe {
                calls: calls.clone(),
                exits: exits.clone(),
                panic,
            }),
        )
        .with_execution_timeout(Duration::from_millis(30));
        (Arc::new(fast), calls, exits)
    }
    fn finish_probe(fast: &FastPolicyConsumer) -> FastPolicyOutcome {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let start = Instant::now();
            let outcome = fast.tick(10, &ExecutionTargets::default());
            assert!(start.elapsed() < Duration::from_millis(50));
            if !matches!(
                outcome,
                FastPolicyOutcome::Pending | FastPolicyOutcome::Cancelling
            ) {
                return outcome;
            }
            assert!(
                Instant::now() < deadline,
                "cooperative executor did not exit"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn p1_fast_execution_timeout_is_terminal_only_after_resource_release() {
        let (fast, calls, exits) = probe_consumer(false);
        for n in 1..=12 {
            assert!(matches!(
                fast.tick(10, &ExecutionTargets::default()),
                FastPolicyOutcome::Pending
            ));
            assert!(matches!(finish_probe(&fast), FastPolicyOutcome::TimedOut));
            assert_eq!(calls.load(Ordering::SeqCst), n);
            assert_eq!(
                exits.load(Ordering::SeqCst),
                n,
                "terminal status preceded cleanup"
            );
        }
    }
    #[test]
    fn p1_fast_execution_cancel_and_drop_release_resources() {
        let (fast, calls, exits) = probe_consumer(false);
        assert!(matches!(
            fast.tick(10, &ExecutionTargets::default()),
            FastPolicyOutcome::Pending
        ));
        let deadline = Instant::now() + Duration::from_secs(1);
        while calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        fast.cancel_execution();
        assert!(matches!(finish_probe(&fast), FastPolicyOutcome::Cancelled));
        assert_eq!(exits.load(Ordering::SeqCst), 1);
        assert!(matches!(
            fast.tick(10, &ExecutionTargets::default()),
            FastPolicyOutcome::Pending
        ));
        while calls.load(Ordering::SeqCst) != 2 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        drop(fast);
        assert_eq!(exits.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn p1_fast_concurrent_polls_do_not_queue_more_executions() {
        let (fast, calls, exits) = probe_consumer(false);
        let deadline = Instant::now() + Duration::from_secs(1);
        fast.tick(10, &ExecutionTargets::default());
        while calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let fast = &fast;
                scope.spawn(move || {
                    for _ in 0..10 {
                        assert!(matches!(
                            fast.tick(10, &ExecutionTargets::default()),
                            FastPolicyOutcome::Pending | FastPolicyOutcome::Cancelling
                        ));
                    }
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        fast.cancel_execution();
        assert!(matches!(
            finish_probe(&fast),
            FastPolicyOutcome::Cancelled | FastPolicyOutcome::TimedOut
        ));
        assert_eq!(exits.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn p1_fast_worker_capacity_is_global_and_reclaimed_after_actual_exit() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let calls = Arc::new(AtomicU64::new(0));
        let exits = Arc::new(AtomicU64::new(0));
        let make = || {
            FastPolicyConsumer::new(
                worker.clone(),
                Arc::new(ResourcePolicy::default()),
                Arc::new(Accept),
                Arc::new(CooperativeProbe {
                    calls: calls.clone(),
                    exits: exits.clone(),
                    panic: false,
                }),
            )
            .with_execution_timeout(Duration::from_secs(5))
        };
        let consumers: Vec<_> = (0..MAX_EXECUTION_WORKERS).map(|_| make()).collect();
        for consumer in &consumers {
            assert!(matches!(
                consumer.tick(10, &ExecutionTargets::default()),
                FastPolicyOutcome::Pending
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while calls.load(Ordering::SeqCst) < MAX_EXECUTION_WORKERS as u64 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let extra = make();
        assert!(
            matches!(extra.tick(10,&ExecutionTargets::default()),FastPolicyOutcome::Failed(message) if message.contains("capacity"))
        );
        for consumer in &consumers {
            consumer.cancel_execution();
        }
        for consumer in &consumers {
            assert!(matches!(
                finish_probe(consumer),
                FastPolicyOutcome::Cancelled
            ));
        }
        assert_eq!(exits.load(Ordering::SeqCst), MAX_EXECUTION_WORKERS as u64);
        assert!(matches!(
            extra.tick(10, &ExecutionTargets::default()),
            FastPolicyOutcome::Pending
        ));
        extra.cancel_execution();
        assert!(matches!(finish_probe(&extra), FastPolicyOutcome::Cancelled));
        drop(consumers);
        drop(extra);
        assert_eq!(EXECUTION_WORKERS.load(Ordering::Acquire), 0);
    }
    #[test]
    fn p1_fast_executor_panic_is_failure_not_noop() {
        let (fast, _, exits) = probe_consumer(true);
        fast.tick(10, &ExecutionTargets::default());
        assert!(matches!(finish_probe(&fast), FastPolicyOutcome::Failed(_)));
        assert_eq!(exits.load(Ordering::SeqCst), 1);
    }

    struct NonCooperative(Arc<AtomicU64>);
    impl CancellablePolicyExecutor for NonCooperative {
        fn execute_cancellable(
            &self,
            _: &ResourcePolicy,
            _: &ExecutionTargets,
            _: &ExecutionCancellation,
        ) -> Result<ExecutionResult, ExecutionStop> {
            self.0.fetch_add(1, Ordering::SeqCst);
            // A finite fault injection, deliberately violates the token contract.
            std::thread::sleep(Duration::from_millis(200));
            Err(ExecutionStop::TimedOut)
        }
    }
    #[test]
    fn p1_uncooperative_executor_is_not_reported_terminated_or_requeued() {
        let calls = Arc::new(AtomicU64::new(0));
        let fast = FastPolicyConsumer::new(
            Arc::new(SlowPolicyWorker::new().unwrap()),
            Arc::new(ResourcePolicy::default()),
            Arc::new(Accept),
            Arc::new(NonCooperative(calls.clone())),
        )
        .with_execution_timeout(Duration::from_millis(5));
        fast.tick(10, &ExecutionTargets::default());
        let deadline = Instant::now() + Duration::from_secs(1);
        while calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        std::thread::sleep(Duration::from_millis(10));
        for _ in 0..100 {
            assert!(matches!(
                fast.tick(10, &ExecutionTargets::default()),
                FastPolicyOutcome::Cancelling
            ));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(finish_probe(&fast), FastPolicyOutcome::TimedOut));
    }

    #[test]
    fn p1_zero_deadline_never_invokes_executor_and_generation_cancel_exits() {
        let (fast, calls, exits) = probe_consumer(false);
        fast.tick(10, &ExecutionTargets::default());
        let deadline = Instant::now() + Duration::from_secs(1);
        while calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        fast.worker.cancel();
        assert!(matches!(finish_probe(&fast), FastPolicyOutcome::Cancelled));
        assert_eq!(exits.load(Ordering::SeqCst), 1);
        drop(fast);
        let fast = FastPolicyConsumer::new(
            Arc::new(SlowPolicyWorker::new().unwrap()),
            Arc::new(ResourcePolicy::default()),
            Arc::new(Accept),
            Arc::new(CooperativeProbe {
                calls: calls.clone(),
                exits,
                panic: false,
            }),
        )
        .with_execution_timeout(Duration::ZERO);
        fast.tick(10, &ExecutionTargets::default());
        assert!(matches!(finish_probe(&fast), FastPolicyOutcome::TimedOut));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    fn selected_id(fast: &FastPolicyConsumer, now: u64) -> String {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match fast.tick(now, &ExecutionTargets::default()) {
                FastPolicyOutcome::Executed(result) => {
                    assert_eq!(result.overall_status, ExecutionOverallStatus::NoOp);
                    return result.policy_id;
                },
                FastPolicyOutcome::Rejected => panic!("mock validator rejected"),
                FastPolicyOutcome::Pending => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                },
                _ => panic!("model execution did not complete"),
            }
        }
    }

    fn await_prepared(fast: &FastPolicyConsumer) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if selected_id(fast, 10) == "prepared" {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("slow worker never delivered prepared policy");
    }

    #[test]
    fn hung_slow_job_does_not_break_fast_deadline_or_execute_late_result() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        let (started_tx, started_rx) = mpsc::channel();
        assert!(worker.submit(move || {
            let _ = started_tx.send(());
            std::thread::sleep(Duration::from_millis(300));
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "late".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 100,
            })
        }));
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let started = Instant::now();
        assert_eq!(selected_id(&fast, 10), "fallback");
        assert!(started.elapsed() < Duration::from_millis(150));
        worker.cancel();
        std::thread::sleep(Duration::from_millis(320));
        assert_eq!(selected_id(&fast, 10), "fallback");
    }

    #[test]
    fn expired_proposal_falls_back_without_counting_model_as_success() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        assert!(worker.submit(|| {
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "prepared".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 20,
            })
        }));
        await_prepared(&fast);
        assert_eq!(selected_id(&fast, 21), "fallback");
    }

    #[test]
    fn cancellation_revokes_a_previously_delivered_policy() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        assert!(worker.submit(|| {
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "prepared".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 100,
            })
        }));
        await_prepared(&fast);
        worker.cancel();
        assert_eq!(selected_id(&fast, 20), "fallback");
    }

    #[test]
    fn daemon_model_driver_does_not_wait_for_slow_telemetry_or_apply_late_plan() {
        let (loop_, environment) = model_loop(Duration::from_millis(250));
        let driver = ModelTickDriver::new(
            loop_.clone(),
            ModelTargetProvider::default().execution_targets(),
            Duration::from_millis(50),
        )
        .unwrap();
        let started = Instant::now();
        let first = driver.tick();
        assert_eq!(first.execution_status, Some(ExecutionOverallStatus::NoOp));
        assert!(started.elapsed() < Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(280));
        let second = driver.tick();
        assert_eq!(second.tick_id, first.tick_id + 1);
        let metrics = loop_.metrics_collector().snapshot();
        assert_eq!(metrics.failed_ticks, 0);
        assert!(metrics.noop_ticks >= 2);
        assert!(metrics.last_fast_control_us.is_some());
        assert!(metrics.last_slow_prepare_us.unwrap_or_default() >= 250_000);
        assert_eq!(second.execution_status, Some(ExecutionOverallStatus::NoOp));
        assert_eq!(loop_.safe_mode().current_mode(), RuntimeMode::Normal);
        assert!(environment.snapshot().last_policy_id.is_none());
    }

    #[test]
    fn daemon_model_driver_applies_fresh_prepared_policy() {
        let (loop_, environment) = model_loop(Duration::ZERO);
        let driver = ModelTickDriver::new(
            loop_,
            ModelTargetProvider::default().execution_targets(),
            Duration::from_millis(200),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut previous_id = 0;
        while Instant::now() < deadline {
            let outcome = driver.tick();
            assert!(outcome.tick_id > previous_id);
            previous_id = outcome.tick_id;
            if outcome.validation_accepted {
                assert_eq!(
                    outcome.execution_status,
                    Some(ExecutionOverallStatus::AllSucceeded)
                );
                assert!(environment.snapshot().last_policy_id.is_some());
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("model preparation never reached the apply side");
    }

    #[test]
    fn platform_driver_never_binds_an_expired_plan() {
        let calls = Arc::new(AtomicU64::new(0));
        let loop_ = platform_mock_loop(Duration::from_millis(250), calls.clone());
        let driver = PlatformTickDriver::new(
            loop_.clone(),
            ExecutionTargets::default(),
            Duration::from_millis(50),
        )
        .unwrap();
        let started = Instant::now();
        assert_eq!(
            driver.tick().execution_status,
            Some(ExecutionOverallStatus::NoOp)
        );
        assert!(started.elapsed() < Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(280));
        assert_eq!(
            driver.tick().execution_status,
            Some(ExecutionOverallStatus::NoOp)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(loop_.safe_mode().current_mode(), RuntimeMode::Normal);
    }

    #[test]
    fn platform_driver_binds_only_after_a_fresh_plan_is_consumed() {
        let calls = Arc::new(AtomicU64::new(0));
        let loop_ = platform_mock_loop(Duration::ZERO, calls.clone());
        let driver = PlatformTickDriver::new(
            loop_,
            ExecutionTargets::default(),
            Duration::from_millis(500),
        )
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let outcome = driver.tick();
            if outcome.validation_accepted {
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(outcome.execution_status, Some(ExecutionOverallStatus::NoOp));
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("platform mock plan was never consumed");
    }
}
