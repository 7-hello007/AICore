use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use aicore_contracts::{
    execution::{result::ExecutionResult, targets::ExecutionTargets},
    memory::record::HistoricalRecord,
    policy::resource_policy::ResourcePolicy,
    runtime::tick::TickOutcome,
    traits::{policy_executor::IPolicyExecutor, policy_validator::IPolicyValidator},
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
}

/// Consumes only precomputed policies. It does no telemetry, history, AI or
/// runtime binding and never waits for the slow worker. The fallback is a
/// caller-supplied rule policy, not a fabricated successful execution.
pub struct FastPolicyConsumer {
    worker: Arc<SlowPolicyWorker>,
    current: Mutex<Option<PreparedEnvelope>>,
    fallback: Arc<ResourcePolicy>,
    validator: Arc<dyn IPolicyValidator>,
    executor: Arc<dyn IPolicyExecutor>,
}

impl FastPolicyConsumer {
    pub fn new(
        worker: Arc<SlowPolicyWorker>,
        fallback: Arc<ResourcePolicy>,
        validator: Arc<dyn IPolicyValidator>,
        executor: Arc<dyn IPolicyExecutor>,
    ) -> Self {
        Self {
            worker,
            current: Mutex::new(None),
            fallback,
            validator,
            executor,
        }
    }

    pub fn tick(&self, now_unix_ms: u64, targets: &ExecutionTargets) -> FastPolicyOutcome {
        if let Some(proposal) = self.worker.latest() {
            if let Ok(mut current) = self.current.lock() {
                *current = Some(proposal);
            }
        }
        let policy = self
            .current
            .lock()
            .ok()
            .and_then(|current| current.clone())
            .filter(|envelope| {
                envelope.generation == self.worker.generation.load(Ordering::SeqCst)
                    && envelope.proposal.expires_at_unix_ms > now_unix_ms
            })
            .map(|envelope| envelope.proposal.policy)
            .unwrap_or_else(|| self.fallback.clone());
        if !self.validator.validate(&policy).accepted {
            return FastPolicyOutcome::Rejected;
        }
        FastPolicyOutcome::Executed(self.executor.execute(&policy, targets))
    }
}

/// Daemon-facing split of the existing AdaptiveLoop. A single bounded worker
/// performs slow preparation. The scheduler thread consumes only ready plans
/// or records a neutral NoOp. Model binding is safe on the slow worker.
pub struct ModelTickDriver {
    loop_: Arc<AdaptiveLoop>,
    targets: ExecutionTargets,
    requests: mpsc::SyncSender<()>,
    results: mpsc::Receiver<Result<PreparedTick, Box<TickOutcome>>>,
    history_sender: Option<mpsc::SyncSender<HistoricalRecord>>,
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
                    // A full result channel drops the older plan. Its task
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
        Ok(Self {
            loop_,
            targets,
            requests,
            results,
            history_sender,
            published_ticks: AtomicU64::new(0),
        })
    }

    pub fn tick(&self) -> TickOutcome {
        let start = std::time::Instant::now();
        let ready = self.results.try_recv().ok();
        let _ = self.requests.try_send(());
        let mut outcome = match ready {
            Some(Ok(plan)) => self.loop_.apply_prepared_tick_with_history_sender(
                plan,
                &self.targets,
                self.history_sender.as_ref(),
            ),
            Some(Err(outcome)) => *outcome,
            None => self.loop_.neutral_tick("slow model preparation pending"),
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

/// Platform split driver. Slow providers cannot delay polling a ready plan.
/// Hardware/runtime binding stays on the apply side after expiry and SafeMode
/// checks, since cancelling a late hardware write is impossible. Its latency
/// still contributes to the control-side duration and must be measured on a
/// target host before claiming a hard deadline.
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

    fn selected_id(outcome: FastPolicyOutcome) -> String {
        match outcome {
            FastPolicyOutcome::Executed(result) => {
                assert_eq!(result.overall_status, ExecutionOverallStatus::NoOp);
                result.policy_id
            },
            FastPolicyOutcome::Rejected => panic!("mock validator rejected"),
        }
    }

    fn await_prepared(fast: &FastPolicyConsumer) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if selected_id(fast.tick(10, &ExecutionTargets::default())) == "prepared" {
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
        assert_eq!(
            selected_id(fast.tick(10, &ExecutionTargets::default())),
            "fallback"
        );
        assert!(started.elapsed() < Duration::from_millis(150));
        worker.cancel();
        std::thread::sleep(Duration::from_millis(320));
        assert_eq!(
            selected_id(fast.tick(10, &ExecutionTargets::default())),
            "fallback"
        );
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
        assert_eq!(
            selected_id(fast.tick(21, &ExecutionTargets::default())),
            "fallback"
        );
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
        assert_eq!(
            selected_id(fast.tick(20, &ExecutionTargets::default())),
            "fallback"
        );
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
