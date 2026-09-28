use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    context::workload::WorkloadType,
    execution::{status::ExecutionOverallStatus, targets::ExecutionTargets},
    memory::context::MemoryContext,
    memory::{profile::UserProfile, record::HistoricalRecord},
    policy::resource_policy::ResourcePolicy,
    prediction::input::PredictionInput,
    runtime::{
        config::AdaptiveLoopConfig, metrics::LoopMetrics, mode::RuntimeMode, tick::TickOutcome,
        trace::TraceContext,
    },
    traits::{
        memory_store::IMemoryStore,
        policy_engine::IPolicyEngine,
        policy_executor::IPolicyExecutor,
        policy_validator::IPolicyValidator,
        predictor::IPredictor,
        runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
        runtime_orchestrator::IRuntimeOrchestrator,
        runtime_router::IRuntimeRouter,
        task_queue::ITaskQueue,
        telemetry_provider::ITelemetryProvider,
        transparent_executor::ITransparentExecutor,
    },
};
use aicore_tier::TierHub;

use aicore_core::{
    context::ContextEngine,
    event_bus::{Event, EventBus},
    normalizer::TelemetryNormalizer,
    policy::DefaultPolicyValidator,
};

use crate::metrics::MetricsCollector;
use crate::safe_mode::SafeModeController;

static RECORD_ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Immutable result of the slow observation/planning side. Only a validated
/// policy reaches the apply side; dropping an unused plan fails its dequeued
/// tasks instead of leaving them Running forever.
pub struct PreparedTick {
    tick_id: u64,
    trace: TraceContext,
    start: Instant,
    max_age: Option<Duration>,
    mode_before: RuntimeMode,
    state: aicore_core::domain::computer_state::ComputerState,
    context: aicore_contracts::context::snapshot::ContextSnapshot,
    prediction: Option<aicore_contracts::prediction::output::Prediction>,
    application_id: Option<aicore_contracts::application::id::ApplicationId>,
    tasks: PreparedTaskGuard,
    runtime_route: Option<aicore_contracts::runtime::route::RuntimeRoute>,
    runtime_binding: Option<BindingResult>,
    binding_deferred: bool,
    policy: ResourcePolicy,
    snap_fields: SnapshotFields,
}

struct PreparedTaskGuard {
    tasks: Vec<aicore_contracts::task::queued::QueuedTask>,
    queue: Option<Arc<dyn ITaskQueue>>,
    armed: bool,
}

impl Drop for PreparedTaskGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Some(queue) = &self.queue {
                for task in &self.tasks {
                    if let Err(error) =
                        queue.fail(&task.task_id, "prepared policy expired or discarded".into())
                    {
                        tracing::warn!(component="adaptive_loop", event="prepared_task_cleanup_failed", task_id=%task.task_id, error=%error);
                    }
                }
            }
        }
    }
}

fn historical_record_id(timestamp_unix_ms: u64, tick_id: u64) -> String {
    let sequence = RECORD_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "tick-{}-{}-{}-{}",
        timestamp_unix_ms,
        std::process::id(),
        tick_id,
        sequence
    )
}

/// Top-level adaptive loop.
///
/// Each tick completes a control loop:
/// Observe → Understand → Decide → Validate → Execute → Measure → Learn。
pub struct AdaptiveLoop {
    telemetry: Arc<dyn ITelemetryProvider>,
    normalizer: TelemetryNormalizer,
    context_engine: Arc<ContextEngine>,
    predictor: Option<Arc<dyn IPredictor>>,
    primary_engine: Arc<dyn IPolicyEngine>,
    fallback_engine: Arc<dyn IPolicyEngine>,
    validator: Arc<dyn IPolicyValidator>,
    executor: Arc<dyn IPolicyExecutor>,
    transparent: Option<Arc<dyn ITransparentExecutor>>,
    task_queue: Option<Arc<dyn ITaskQueue>>,
    runtime_router: Option<Arc<dyn IRuntimeRouter>>,
    runtime_binder: Option<Arc<dyn IRuntimeBinder>>,
    tier_hub: Option<Arc<TierHub>>,
    event_bus: Option<Arc<EventBus>>,
    last_applied_policy: Mutex<Option<ResourcePolicy>>,
    memory: Option<Arc<dyn IMemoryStore>>,
    safe_mode: Arc<SafeModeController>,
    metrics: Arc<MetricsCollector>,
    thermal_guard: Option<Arc<aicore_core::policy::ThermalPowerGuard>>,
    effect_feedback: Option<Arc<aicore_core::policy::FeedbackPolicyEngine>>,
    observed_effects_enabled: bool,
    tick_counter: AtomicU64,
}

impl AdaptiveLoop {
    pub fn new(
        telemetry: Arc<dyn ITelemetryProvider>,
        context_engine: Arc<ContextEngine>,
        primary_engine: Arc<dyn IPolicyEngine>,
        fallback_engine: Arc<dyn IPolicyEngine>,
        validator: Arc<dyn IPolicyValidator>,
        executor: Arc<dyn IPolicyExecutor>,
        config: AdaptiveLoopConfig,
    ) -> Self {
        Self {
            telemetry,
            normalizer: TelemetryNormalizer::new(),
            context_engine,
            predictor: None,
            primary_engine,
            fallback_engine,
            validator,
            executor,
            transparent: None,
            task_queue: None,
            runtime_router: None,
            runtime_binder: None,
            tier_hub: None,
            event_bus: None,
            last_applied_policy: Mutex::new(None),
            memory: None,
            safe_mode: Arc::new(SafeModeController::new(config)),
            metrics: Arc::new(MetricsCollector::new()),
            thermal_guard: None,
            effect_feedback: None,
            observed_effects_enabled: false,
            tick_counter: AtomicU64::new(0),
        }
    }

    /// Optionally enable memory writes, subject to the user profile's consent.
    pub fn with_memory(mut self, memory: Arc<dyn IMemoryStore>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Optionally enable short-term prediction.
    pub fn with_predictor(mut self, predictor: Arc<dyn IPredictor>) -> Self {
        self.predictor = Some(predictor);
        self
    }

    pub fn with_thermal_guard(
        mut self,
        guard: Arc<aicore_core::policy::ThermalPowerGuard>,
    ) -> Self {
        self.thermal_guard = Some(guard);
        self
    }

    /// Optionally enable transparent execution (six stages and rollback).
    pub fn with_transparent_executor(mut self, exec: Arc<dyn ITransparentExecutor>) -> Self {
        self.transparent = Some(exec);
        self
    }

    /// Optional: Enable the Task queue.
    pub fn with_task_queue(mut self, queue: Arc<dyn ITaskQueue>) -> Self {
        self.task_queue = Some(queue);
        self
    }

    /// Optional: Enable Runtime Router.
    pub fn with_runtime_router(mut self, router: Arc<dyn IRuntimeRouter>) -> Self {
        self.runtime_router = Some(router);
        self
    }

    /// Optional: Enable Runtime Binder.
    pub fn with_runtime_binder(mut self, binder: Arc<dyn IRuntimeBinder>) -> Self {
        self.runtime_binder = Some(binder);
        self
    }

    /// Inject an independent TierHub; Core discovers L0/L1/L2 through capabilities.
    pub fn with_tier_hub(mut self, hub: Arc<TierHub>) -> Self {
        self.tier_hub = Some(hub);
        self
    }

    /// Inject an event bus that publishes observations without making decisions.
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn safe_mode(&self) -> &Arc<SafeModeController> {
        &self.safe_mode
    }

    pub fn metrics_collector(&self) -> &Arc<MetricsCollector> {
        &self.metrics
    }

    pub(crate) fn has_model_only_effects(&self) -> bool {
        self.executor.name() == "model_policy_executor"
            && self
                .runtime_binder
                .as_ref()
                .is_some_and(|binder| binder.name() == "model_runtime_binder")
    }

    pub(crate) fn memory_store(&self) -> Option<Arc<dyn IMemoryStore>> {
        self.memory.clone()
    }

    pub(crate) fn may_record_history(memory: &Arc<dyn IMemoryStore>) -> bool {
        Self::should_record_history(memory)
    }

    fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn should_record_history(memory: &Arc<dyn IMemoryStore>) -> bool {
        memory
            .load_profile()
            .map(|p: UserProfile| p.can_record())
            .unwrap_or(false)
    }

    /// Construct MemoryContext from memory store.
    ///
    /// - No memory: None.
    /// - Neither history nor personalization is permitted: None (privacy first).
    /// - Memory is permitted: load and aggregate the latest N records.
    fn build_memory_context(&self) -> Option<aicore_contracts::memory::context::MemoryContext> {
        let memory = self.memory.as_ref()?;

        aicore_memory::personalized_context(memory.as_ref(), 100)
            .ok()
            .flatten()
    }

    /// Apply short-term prediction.
    ///
    /// Returns (merged_context, prediction).
    /// - No predictor: preserve the original context and return None.
    /// - Failed or low-confidence prediction: preserve the original context and return None.
    /// - Successful prediction: replace workload_type and return Some(prediction).
    fn apply_prediction(
        &self,
        context: aicore_contracts::context::snapshot::ContextSnapshot,
        state: &aicore_core::domain::computer_state::ComputerState,
        memory_ctx: &Option<MemoryContext>,
        application_id: Option<aicore_contracts::application::id::ApplicationId>,
    ) -> (
        aicore_contracts::context::snapshot::ContextSnapshot,
        Option<aicore_contracts::prediction::output::Prediction>,
    ) {
        let predictor = match &self.predictor {
            Some(p) => p,
            None => return (context, None),
        };

        let input = PredictionInput {
            application_id,
            snapshot: state.snapshot.clone(),
            context: context.clone(),
            memory: memory_ctx.clone(),
            horizon_ms: predictor.horizon_ms(),
        };

        match predictor.predict(&input) {
            Ok(pred) if pred.confidence >= 0.5 => {
                let mut merged = context;
                merged.workload_type = pred.predicted_workload;
                (merged, Some(pred))
            },
            _ => (context, None),
        }
    }

    /// Dequeue a bounded batch per tick; the queue performs the dequeue atomically.
    fn dequeue_tasks(&self, limit: usize) -> Vec<aicore_contracts::task::queued::QueuedTask> {
        let queue = match self.task_queue.as_ref() {
            Some(queue) => queue,
            None => return Vec::new(),
        };
        match queue.dequeue_batch(limit.max(1)) {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::warn!(
                    component = "adaptive_loop",
                    event = "dequeue_failed",
                    error = %e,
                    "failed to dequeue tasks"
                );
                Vec::new()
            },
        }
    }

    /// Collects fields for Dashboard from tick.
    fn collect_snapshot_fields(
        &self,
        state: &aicore_core::domain::computer_state::ComputerState,
        context: &aicore_contracts::context::snapshot::ContextSnapshot,
        policy: &aicore_contracts::policy::resource_policy::ResourcePolicy,
        route: Option<&aicore_contracts::runtime::route::RuntimeRoute>,
    ) -> SnapshotFields {
        let snap = &state.snapshot;
        SnapshotFields {
            cpu_usage_percent: Some(snap.cpu.usage_percent),
            gpu_usage_percent: snap.gpu.usage_percent,
            npu_usage_percent: snap.npu.usage_percent,
            memory_usage_percent: Some(snap.memory.usage_percent),
            current_context_json: serde_json::to_string(context).ok(),
            current_policy_json: serde_json::to_string(policy).ok(),
            runtime_route_json: route.and_then(|r| serde_json::to_string(r).ok()),
            ai_runtime_health_json: self.ai_runtime_health_json(),
        }
    }

    /// Generates a health summary of AI Runtime from runtime binder.
    fn ai_runtime_health_json(&self) -> Option<String> {
        let binder = self.runtime_binder.as_ref()?;
        let models = binder.loaded_models();
        let summary = aicore_contracts::dashboard::snapshot::AiRuntimeHealthSummary {
            runtime_name: binder.name().to_string(),
            healthy: !models.is_empty(),
            loaded_models: models.len(),
            degraded_reason: if models.is_empty() {
                Some("no model loaded".to_string())
            } else {
                None
            },
        };
        serde_json::to_string(&summary).ok()
    }

    /// Decide the runtime route without system side effects.
    fn decide_runtime_route(
        &self,
        state: &aicore_core::domain::computer_state::ComputerState,
        context: &aicore_contracts::context::snapshot::ContextSnapshot,
    ) -> Option<aicore_contracts::runtime::route::RuntimeRoute> {
        let router = self.runtime_router.as_ref()?;
        match router.decide(&state.snapshot, context) {
            Ok(route) => Some(route),
            Err(e) => {
                tracing::warn!(component="adaptive_loop", event="route_decide_failed", error=%e, "router.decide failed");
                None
            },
        }
    }

    /// Only execute the binder after the policy passes validation.
    fn execute_runtime_binding(
        &self,
        route: Option<&aicore_contracts::runtime::route::RuntimeRoute>,
        state: &aicore_core::domain::computer_state::ComputerState,
        context: &aicore_contracts::context::snapshot::ContextSnapshot,
    ) -> Option<BindingResult> {
        let route = route?;
        let binder = self.runtime_binder.as_ref()?;
        // A missing metric for a discovered device is unknown, not zero.
        // Zero is meaningful only when that device is actually absent.
        let inference =
            if route.source != aicore_contracts::runtime::route::RuntimeRouteSource::Native {
                (|| {
                    let utilization = |name: &Option<String>, measured: Option<f32>| {
                        measured.or_else(|| name.is_none().then_some(0.0))
                    };
                    let values = [
                        state.snapshot.cpu.usage_percent,
                        utilization(&state.snapshot.gpu.name, state.snapshot.gpu.usage_percent)?,
                        utilization(&state.snapshot.npu.name, state.snapshot.npu.usage_percent)?,
                        state.snapshot.memory.usage_percent,
                    ];
                    if values.iter().any(|v| !v.is_finite()) {
                        return None;
                    }
                    Some(aicore_contracts::ai::runtime::InferenceRequest {
                        features: values
                            .into_iter()
                            .map(|v| (v / 100.0).clamp(0.0, 1.0))
                            .collect(),
                        tensors: Vec::new(),
                    })
                })()
            } else {
                None
            };
        if inference.is_none()
            && route.source != aicore_contracts::runtime::route::RuntimeRouteSource::Native
        {
            tracing::debug!(
                component = "adaptive_loop",
                event = "binding_inference_unavailable",
                "required utilization not observed; binder must decline inference"
            );
        }
        let req = BindingRequest {
            route: route.clone(),
            context: context.clone(),
            inference,
        };
        match binder.bind_and_execute(&req) {
            Ok(binding) => Some(binding),
            Err(e) => {
                tracing::warn!(component="adaptive_loop", event="binding_failed", error=%e, "binder.bind_and_execute failed after validation");
                None
            },
        }
    }
}

impl AdaptiveLoop {
    pub fn with_effect_feedback(
        mut self,
        engine: Arc<aicore_core::policy::FeedbackPolicyEngine>,
    ) -> Self {
        self.effect_feedback = Some(engine);
        self
    }
    pub fn with_observed_effects_enabled(mut self, enabled: bool) -> Self {
        self.observed_effects_enabled = enabled;
        self
    }
    /// Trusted collector ingestion. Only the last successfully applied policy
    /// may receive evidence; simulated readings never alter the next policy.
    pub fn record_effect(
        &self,
        comparison: &aicore_contracts::effect::EffectComparison,
        budget: aicore_contracts::effect::OverheadBudget,
    ) -> aicore_contracts::errors::ContractResult<aicore_contracts::effect::EffectAssessment> {
        use aicore_contracts::errors::ContractError;
        if !self.observed_effects_enabled || self.has_model_only_effects() {
            return Err(ContractError::NotSupported(
                "model execution cannot accept observed policy effects".into(),
            ));
        }
        let engine = self
            .effect_feedback
            .as_ref()
            .ok_or_else(|| ContractError::NotSupported("feedback engine not configured".into()))?;
        let guard = self
            .last_applied_policy
            .lock()
            .map_err(|_| ContractError::LockPoisoned("last policy lock poisoned".into()))?;
        let policy = guard
            .as_ref()
            .ok_or_else(|| ContractError::InvalidRequest("no applied policy".into()))?;
        engine.record_effect(policy, comparison, budget, Self::now_unix_ms())
    }
    /// Slow side for model execution: telemetry, history, prediction, policy
    /// validation and model binding. Platform binding can change device state
    /// and must instead be deferred until a fresh plan reaches the apply side.
    pub fn prepare_tick(
        &self,
        max_age: Option<Duration>,
    ) -> Result<PreparedTick, Box<TickOutcome>> {
        self.prepare_tick_inner(max_age, false)
    }

    pub(crate) fn prepare_platform_tick(
        &self,
        max_age: Duration,
    ) -> Result<PreparedTick, Box<TickOutcome>> {
        self.prepare_tick_inner(
            Some(max_age),
            !self
                .runtime_binder
                .as_ref()
                .is_some_and(|b| b.preparation_safe()),
        )
    }

    fn prepare_tick_inner(
        &self,
        max_age: Option<Duration>,
        defer_binding: bool,
    ) -> Result<PreparedTick, Box<TickOutcome>> {
        let tick_id = self.tick_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let trace = TraceContext::new(tick_id);
        let start = Instant::now();
        let mode_before = self.safe_mode.current_mode();

        tracing::info!(
            component = "adaptive_loop",
            event = "tick_start",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            mode = ?mode_before,
            "tick start"
        );

        // 1. Observe
        let snapshot = match self.telemetry.collect() {
            Ok(s) => s,
            Err(e) => {
                let mode_after = self.safe_mode.record_failure();
                let duration_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_tick(false, duration_ms);

                tracing::warn!(
                    component = "adaptive_loop",
                    event = "telemetry_failed",
                    trace_id = %trace.trace_id,
                    tick_id = tick_id,
                    error = %e,
                    duration_ms = duration_ms,
                    "telemetry collect failed"
                );

                return Err(Box::new(TickOutcome {
                    tick_id,
                    trace_id: trace.trace_id.clone(),
                    timestamp_unix_ms: Self::now_unix_ms(),
                    duration_ms,
                    success: false,
                    mode_after,
                    workload_type: WorkloadType::Unknown,
                    policy_id: None,
                    policy_source: None,
                    validation_accepted: false,
                    execution_status: None,
                    message: format!("telemetry collect failed: {}", e),
                    predicted_workload: None,
                    prediction_confidence: None,
                    runtime_route_source: None,
                    runtime_binding_executed: None,
                    runtime_binding_reason: None,
                    cpu_usage_percent: None,
                    gpu_usage_percent: None,
                    npu_usage_percent: None,
                    memory_usage_percent: None,
                    current_context_json: None,
                    current_policy_json: None,
                    runtime_route_json: None,
                    ai_runtime_health_json: None,
                }));
            },
        };

        // 2. Understand
        let state = self.normalizer.normalize(snapshot);
        if let Some(bus) = &self.event_bus {
            bus.publish(&Event::SystemStateUpdated(
                aicore_contracts::events::system_state_updated::SystemStateUpdated {
                    version: "1.0".to_string(),
                    timestamp_unix_ms: Self::now_unix_ms(),
                    snapshot: state.snapshot.clone(),
                },
            ));
        }
        let tier_workload_source = self
            .tier_hub
            .as_ref()
            .and_then(|hub| hub.best_workload_source().ok().flatten());
        let context = self
            .context_engine
            .build_with_workload_source(&state, tier_workload_source);

        tracing::debug!(
            component = "adaptive_loop",
            event = "context_built",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            workload = ?context.workload_type,
            interaction = ?context.interaction_level,
            confidence = context.confidence,
            "context built"
        );

        // 2.5: An explicit L1 task takes priority over L0 inference; L2 registers separately.
        let tier_task = self.tier_hub.as_ref().and_then(|hub| {
            hub.best_task_source()
                .ok()
                .flatten()
                .and_then(|src| src.current_task().ok().flatten())
        });
        // A tick performs only one task to avoid that one policy is wrongly used for multiple tasks before all is marked.
        const MAX_TASKS_PER_TICK: usize = 1;
        let mut tasks = PreparedTaskGuard {
            tasks: self.dequeue_tasks(MAX_TASKS_PER_TICK),
            queue: self.task_queue.clone(),
            armed: true,
        };
        let active_tasks = &tasks.tasks;
        let active_task = active_tasks.first();
        let application_id = tier_task
            .as_ref()
            .or_else(|| active_task.map(|task| &task.contract))
            .map(|contract| contract.application_id.clone());
        let memory_ctx = self.build_memory_context();
        let (mut context, prediction) =
            self.apply_prediction(context, &state, &memory_ctx, application_id.clone());
        if let Some(contract) = tier_task.as_ref() {
            context = apply_task_contract(context, contract);
        }
        if let Some(task) = active_task {
            context = apply_task_hints(context, task);
        }
        if !active_tasks.is_empty() {
            tracing::info!(
                component="adaptive_loop",
                event="tasks_dequeued",
                trace_id=%trace.trace_id,
                tick_id=tick_id,
                task_count=active_tasks.len(),
                task_ids=?active_tasks.iter().map(|t| t.task_id.as_str()).collect::<Vec<_>>(),
                "tasks dequeued"
            );
        }
        if let Some(bus) = &self.event_bus {
            bus.publish(&Event::ContextUpdated(
                aicore_contracts::events::context_updated::ContextUpdated {
                    version: "1.0".to_string(),
                    timestamp_unix_ms: Self::now_unix_ms(),
                    context: context.clone(),
                },
            ));
        }

        let runtime_route = self.decide_runtime_route(&state, &context);
        let runtime_binding: Option<BindingResult> = None;
        tracing::debug!(
            component = "adaptive_loop",
            event = "runtime_routing",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            route_source = ?runtime_route.as_ref().map(|r| r.source.as_str()),
            binding_executed = ?runtime_binding.as_ref().map(|b| b.executed),
            "runtime routing complete"
        );

        // 3. Decide: select the engine for the current mode.
        let engine: &Arc<dyn IPolicyEngine> = match mode_before {
            RuntimeMode::Normal => &self.primary_engine,
            RuntimeMode::Degraded | RuntimeMode::Safe | RuntimeMode::Recovery => {
                &self.fallback_engine
            },
        };

        let mut policy = engine.generate_with_memory(&state.snapshot, &context, memory_ctx);
        self.metrics.record_policy_generated();

        if let Some(provider) = self
            .tier_hub
            .as_ref()
            .and_then(|hub| hub.best_adaptation_provider().ok().flatten())
        {
            match provider.suggest(&state.snapshot, &context) {
                Ok(Some(hint)) => {
                    if let Some(override_policy) = hint.policy_override {
                        tracing::debug!(component="adaptive_loop", event="tier_policy_override", source=provider.source_name(), reason=%hint.reason, confidence=hint.confidence, "tier override applied");
                        policy = override_policy;
                    }
                },
                Ok(None) => {},
                Err(err) => {
                    tracing::debug!(component="adaptive_loop", event="tier_adaptation_failed", source=provider.source_name(), error=%err, "tier adaptation unavailable; keeping engine policy")
                },
            }
        }

        // TaskContract is an explicit application constraint that must be re-imposed after any Tier policy override.
        if let Some(contract) = tier_task
            .as_ref()
            .or_else(|| active_task.map(|task| &task.contract))
        {
            apply_task_contract_to_policy(&mut policy, contract);
        }

        if let Some(guard) = &self.thermal_guard {
            guard.constrain(&state.snapshot, &mut policy);
        }

        // V4: collect Dashboard data-plane fields.
        let snap_fields =
            self.collect_snapshot_fields(&state, &context, &policy, runtime_route.as_ref());

        tracing::debug!(
            component = "adaptive_loop",
            event = "policy_generated",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            policy_id = %policy.policy_id,
            source = ?policy.source,
            priority = ?policy.priority,
            "policy generated"
        );

        if let Some(bus) = &self.event_bus {
            bus.publish(&Event::PolicyUpdated(
                aicore_contracts::events::policy_updated::PolicyUpdated {
                    version: "1.0".to_string(),
                    timestamp_unix_ms: Self::now_unix_ms(),
                    policy: policy.clone(),
                },
            ));
        }

        // 4. Validate
        let validation = self.validator.validate(&policy);
        if !validation.accepted {
            self.metrics.record_policy_rejected();
            if let Some(queue) = &self.task_queue {
                for task in active_tasks {
                    if let Err(error) = queue.fail(
                        &task.task_id,
                        format!("policy rejected: {:?}", validation.issues),
                    ) {
                        tracing::warn!(
                            component = "adaptive_loop",
                            event = "task_finish_failed",
                            task_id = %task.task_id,
                            error = %error,
                            "failed to mark rejected task failed"
                        );
                    }
                }
            }
            tasks.armed = false;
            let mode_after = self.safe_mode.record_failure();
            let duration_ms = start.elapsed().as_millis() as u64;
            self.metrics.record_tick(false, duration_ms);

            tracing::warn!(
                component = "adaptive_loop",
                event = "policy_rejected",
                trace_id = %trace.trace_id,
                tick_id = tick_id,
                policy_id = %policy.policy_id,
                issues = ?validation.issues,
                duration_ms = duration_ms,
                "policy rejected by validator"
            );

            return Err(Box::new(TickOutcome {
                tick_id,
                trace_id: trace.trace_id.clone(),
                timestamp_unix_ms: Self::now_unix_ms(),
                duration_ms,
                success: false,
                mode_after,
                workload_type: context.workload_type,
                policy_id: Some(policy.policy_id.clone()),
                policy_source: Some(policy.source),
                validation_accepted: false,
                execution_status: None,
                message: format!("policy rejected: {:?}", validation.issues),
                predicted_workload: prediction.as_ref().map(|p| p.predicted_workload),
                prediction_confidence: prediction.as_ref().map(|p| p.confidence),
                runtime_route_source: runtime_route
                    .as_ref()
                    .map(|r| r.source.as_str().to_string()),
                runtime_binding_executed: runtime_binding.as_ref().map(|b| b.executed),
                cpu_usage_percent: snap_fields.cpu_usage_percent,
                gpu_usage_percent: snap_fields.gpu_usage_percent,
                npu_usage_percent: snap_fields.npu_usage_percent,
                memory_usage_percent: snap_fields.memory_usage_percent,
                current_context_json: snap_fields.current_context_json.clone(),
                current_policy_json: snap_fields.current_policy_json.clone(),
                runtime_route_json: snap_fields.runtime_route_json.clone(),
                ai_runtime_health_json: snap_fields.ai_runtime_health_json.clone(),
                runtime_binding_reason: runtime_binding.as_ref().map(|b| b.reason.clone()),
            }));
        }

        // The binder may produce inference or hardware side effects only after policy validation.
        let is_preparation = active_task.is_some_and(|task| {
            self.task_queue.as_ref().is_some_and(|q| {
                q.preparation_targets(&task.task_id)
                    .map(|targets| targets.is_some())
                    .unwrap_or(true)
            })
        });
        let runtime_binding = if defer_binding || is_preparation {
            None
        } else {
            self.execute_runtime_binding(runtime_route.as_ref(), &state, &context)
        };

        Ok(PreparedTick {
            tick_id,
            trace,
            start,
            max_age,
            mode_before,
            state,
            context,
            prediction,
            application_id,
            tasks,
            runtime_route,
            runtime_binding,
            binding_deferred: defer_binding,
            policy,
            snap_fields,
        })
    }

    /// Apply only a fresh, validated immutable plan. Platform plans defer
    /// potentially state-changing binding until after expiry and mode checks.
    pub fn apply_prepared_tick(
        &self,
        prepared: PreparedTick,
        targets: &ExecutionTargets,
    ) -> TickOutcome {
        self.apply_prepared_tick_inner(prepared, targets, None)
    }

    pub(crate) fn apply_prepared_tick_with_history_sender(
        &self,
        prepared: PreparedTick,
        targets: &ExecutionTargets,
        history_sender: Option<&mpsc::SyncSender<HistoricalRecord>>,
    ) -> TickOutcome {
        self.apply_prepared_tick_inner(prepared, targets, history_sender)
    }

    fn apply_prepared_tick_inner(
        &self,
        prepared: PreparedTick,
        targets: &ExecutionTargets,
        history_sender: Option<&mpsc::SyncSender<HistoricalRecord>>,
    ) -> TickOutcome {
        let PreparedTick {
            tick_id,
            trace,
            start,
            max_age,
            mode_before,
            state,
            context,
            prediction,
            application_id,
            mut tasks,
            runtime_route,
            runtime_binding,
            binding_deferred,
            policy,
            snap_fields,
        } = prepared;
        if max_age.is_some_and(|age| start.elapsed() > age)
            || (max_age.is_some() && self.safe_mode.current_mode() != mode_before)
        {
            return self.neutral_tick("prepared policy expired before apply");
        }
        if let Some(queue) = &self.task_queue {
            for task in &tasks.tasks {
                match queue.get(&task.task_id) {
                    Ok(Some(current))
                        if current.state
                            == aicore_contracts::task::lifecycle::TaskState::Running
                            && !current.contract.deadline_unix_secs.is_some_and(|deadline| {
                                Self::now_unix_ms() >= deadline.saturating_mul(1000)
                            }) => {},
                    _ => {
                        tasks.armed = false;
                        return self.neutral_tick("prepared task was revoked before apply");
                    },
                }
            }
        }
        let mut preparation_targets = None;
        if let Some(queue) = &self.task_queue {
            if let Some(task) = tasks.tasks.first() {
                match queue.preparation_targets(&task.task_id) {
                    Ok(targets) => preparation_targets = targets,
                    Err(error) => {
                        return self
                            .neutral_tick(&format!("preparation authorization revoked: {error}"))
                    },
                }
            }
        }
        let runtime_binding = if binding_deferred && preparation_targets.is_none() {
            self.execute_runtime_binding(runtime_route.as_ref(), &state, &context)
        } else {
            runtime_binding
        };
        let active_tasks = tasks.tasks.clone();

        // Platform mode derives targets from this tick's telemetry; explicit targets take precedence.
        let effective_targets = preparation_targets
            .unwrap_or_else(|| derive_execution_targets(targets, &state.snapshot));

        // 5. Execute, preferring the transparent executor.
        let prior = self.last_applied_policy.lock().ok().and_then(|g| g.clone());

        let (exec_result, control_trace) = if let Some(tx) = &self.transparent {
            let trace = tx.execute_transparent(&policy, &effective_targets, prior.as_ref());
            // Preserve Apply details and use the final state after Verify / Rollback.
            let mut result = tx.last_execution_result().unwrap_or_else(|| {
                aicore_contracts::execution::result::ExecutionResult {
                    policy_id: policy.policy_id.clone(),
                    overall_status: trace.final_status,
                    started_at_unix_ms: Self::now_unix_ms(),
                    finished_at_unix_ms: Self::now_unix_ms(),
                    outcomes: Vec::new(),
                }
            });
            result.overall_status = trace.final_status;
            // A successful rollback is NoOp, avoiding a false SafeMode failure.
            if trace.rollback.triggered && trace.rollback.succeeded == Some(true) {
                result.overall_status = ExecutionOverallStatus::NoOp;
            }
            (result, Some(trace))
        } else {
            (self.executor.execute(&policy, &effective_targets), None)
        };

        if let Some(trace) = &control_trace {
            tracing::info!(
                component = "adaptive_loop",
                event = "control_trace",
                trace_id = %trace.trace_id,
                policy_id = %trace.policy_id,
                stages = trace.stages.len(),
                rollback_triggered = trace.rollback.triggered,
                rollback_succeeded = ?trace.rollback.succeeded,
                "transparent control trace"
            );
        }

        let overall = exec_result.overall_status;
        if runtime_binding
            .as_ref()
            .map(|b| b.inference.is_some())
            .unwrap_or(false)
        {
            self.metrics.record_ai_inference();
        }
        if matches!(
            runtime_route.as_ref().map(|r| r.source),
            Some(aicore_contracts::runtime::route::RuntimeRouteSource::AiRuntime)
        ) && runtime_binding
            .as_ref()
            .map(|b| !b.executed)
            .unwrap_or(true)
        {
            self.metrics.record_ai_fallback();
        }
        if let Some(bus) = &self.event_bus {
            bus.publish(&Event::ExecutionCompleted(
                aicore_contracts::events::execution_completed::ExecutionCompleted {
                    version: "1.0".to_string(),
                    timestamp_unix_ms: Self::now_unix_ms(),
                    result: exec_result.clone(),
                },
            ));
        }
        let exec_success = overall.is_success();
        let exec_failure = overall.is_failure();
        self.metrics.record_execution(exec_success);

        // Update last_applied_policy (only when successful)
        if exec_success {
            if let Ok(mut g) = self.last_applied_policy.lock() {
                *g = Some(policy.clone());
            }
        }

        // 6. Measure
        let duration_ms = start.elapsed().as_millis() as u64;
        if exec_success || exec_failure {
            self.metrics.record_tick(exec_success, duration_ms);
        } else {
            self.metrics.record_neutral_tick(duration_ms);
        }

        // 7. SafeMode transition:
        // - Success resets the failure count.
        // - Real failure increments the failure count.
        // - NoOp (unsupported or skipped actions) is neutral.
        let mode_after = if exec_success {
            self.safe_mode.record_success()
        } else if exec_failure {
            self.safe_mode.record_failure()
        } else {
            self.safe_mode.current_mode()
        };
        if matches!(mode_after, RuntimeMode::Safe | RuntimeMode::Recovery)
            && mode_after != mode_before
        {
            self.metrics.record_safe_mode_entry();
        }
        if mode_after != mode_before {
            if let Some(bus) = &self.event_bus {
                bus.publish(&aicore_core::event_bus::Event::RuntimeModeChanged(
                    aicore_contracts::events::runtime_mode_changed::RuntimeModeChanged {
                        version: "1.0".to_string(),
                        timestamp_unix_ms: Self::now_unix_ms(),
                        from: mode_before,
                        to: mode_after,
                        reason: format!("execution result: {:?}", overall),
                    },
                ));
            }
        }

        // 7.5 Task: All tasks consumed by this tick have been completed/failed to avoid long queue build-up.
        if let Some(queue) = &self.task_queue {
            for task in &active_tasks {
                let result = if exec_success {
                    queue.complete(&task.task_id, Some(format!("{:?}", overall)))
                } else if exec_failure {
                    queue.fail(&task.task_id, format!("{:?}", overall))
                } else {
        // NoOp did not execute an action, so the task cannot be marked Completed.
                    queue.fail(&task.task_id, "NoOp: no action executed".to_string())
                };
                if let Err(e) = result {
                    tracing::warn!(
                        component = "adaptive_loop",
                        event = "task_finish_failed",
                        task_id = %task.task_id,
                        error = %e,
                        "failed to mark task finished"
                    );
                }
            }
        }

        // 8. Learn: Writing history (if enabled)
        let mut recorded = false;
        if let Some(memory) = &self.memory {
            if history_sender.is_some() || Self::should_record_history(memory) {
                let timestamp_unix_ms = Self::now_unix_ms();
                let record = HistoricalRecord {
                    record_id: historical_record_id(timestamp_unix_ms, tick_id),
                    application_id,
                    timestamp_unix_ms,
                    workload_type: context.workload_type,
                    interaction_level: context.interaction_level,
                    policy_id: policy.policy_id.clone(),
                    policy_source: policy.source,
                    overall_status: exec_result.overall_status,
                    cpu_usage_percent: state.snapshot.cpu.usage_percent,
                    gpu_usage_percent: state.snapshot.gpu.usage_percent,
                    npu_usage_percent: state.snapshot.npu.usage_percent,
                    memory_usage_percent: state.snapshot.memory.usage_percent,
                    power_mode: policy.power.mode,
                };
                if let Some(sender) = history_sender {
                    if sender.try_send(record).is_err() {
                        tracing::warn!(
                            component = "adaptive_loop",
                            event = "history_queue_full",
                            "history record not queued"
                        );
                    }
                } else if memory.append_record(&record).is_ok() {
                    recorded = true;
                }
            }
        }

        tracing::info!(
            component = "adaptive_loop",
            event = "tick_finished",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            success = exec_success,
            duration_ms = duration_ms,
            mode_after = ?mode_after,
            workload = ?context.workload_type,
            policy_id = %policy.policy_id,
            execution_status = ?exec_result.overall_status,
            recorded = recorded,
            "tick finished"
        );

        tasks.armed = false;
        TickOutcome {
            tick_id,
            trace_id: trace.trace_id.clone(),
            timestamp_unix_ms: Self::now_unix_ms(),
            duration_ms,
            success: exec_success,
            mode_after,
            workload_type: context.workload_type,
            policy_id: Some(policy.policy_id.clone()),
            policy_source: Some(policy.source),
            validation_accepted: true,
            execution_status: Some(exec_result.overall_status),
            message: exec_result.summarize(),
            predicted_workload: prediction.as_ref().map(|p| p.predicted_workload),
            prediction_confidence: prediction.as_ref().map(|p| p.confidence),
            runtime_route_source: runtime_route
                .as_ref()
                .map(|r| r.source.as_str().to_string()),
            runtime_binding_executed: runtime_binding.as_ref().map(|b| b.executed),
            cpu_usage_percent: snap_fields.cpu_usage_percent,
            gpu_usage_percent: snap_fields.gpu_usage_percent,
            npu_usage_percent: snap_fields.npu_usage_percent,
            memory_usage_percent: snap_fields.memory_usage_percent,
            current_context_json: snap_fields.current_context_json.clone(),
            current_policy_json: snap_fields.current_policy_json.clone(),
            runtime_route_json: snap_fields.runtime_route_json.clone(),
            ai_runtime_health_json: snap_fields.ai_runtime_health_json.clone(),
            runtime_binding_reason: runtime_binding.as_ref().map(|b| b.reason.clone()),
        }
    }

    pub fn neutral_tick(&self, reason: &str) -> TickOutcome {
        let tick_id = self.tick_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.metrics.record_neutral_tick(0);
        TickOutcome {
            tick_id,
            trace_id: TraceContext::new(tick_id).trace_id,
            timestamp_unix_ms: Self::now_unix_ms(),
            mode_after: self.safe_mode.current_mode(),
            execution_status: Some(ExecutionOverallStatus::NoOp),
            message: reason.into(),
            ..Default::default()
        }
    }
}

impl IRuntimeOrchestrator for AdaptiveLoop {
    fn tick(&self, targets: &ExecutionTargets) -> TickOutcome {
        match self.prepare_tick(None) {
            Ok(prepared) => self.apply_prepared_tick(prepared, targets),
            Err(outcome) => *outcome,
        }
    }

    fn current_mode(&self) -> RuntimeMode {
        self.safe_mode.current_mode()
    }

    fn metrics(&self) -> LoopMetrics {
        self.metrics.snapshot()
    }

    fn name(&self) -> &'static str {
        "adaptive_loop"
    }
}

impl AdaptiveLoop {
    /// Convenience constructor using the default validator.
    pub fn with_default_validator(
        telemetry: Arc<dyn ITelemetryProvider>,
        context_engine: Arc<ContextEngine>,
        primary_engine: Arc<dyn IPolicyEngine>,
        fallback_engine: Arc<dyn IPolicyEngine>,
        executor: Arc<dyn IPolicyExecutor>,
        config: AdaptiveLoopConfig,
    ) -> Self {
        Self::new(
            telemetry,
            context_engine,
            primary_engine,
            fallback_engine,
            Arc::new(DefaultPolicyValidator::new()),
            executor,
            config,
        )
    }
}

/// Apply Task hints to the context.
///
/// Semantics:
/// - `contract.task` overrides workload_type.
/// - `latency` affects interaction_level.
/// - Only declared fields are overridden; all others retain their original values.
fn apply_task_contract(
    context: aicore_contracts::context::snapshot::ContextSnapshot,
    contract: &aicore_contracts::task::contract::TaskContract,
) -> aicore_contracts::context::snapshot::ContextSnapshot {
    use aicore_contracts::context::workload::InteractionLevel;
    use aicore_contracts::task::preferences::LatencySensitivity;
    let mut context = context;
    if let Some(w) = contract.task {
        context.workload_type = w;
        context.confidence = 1.0;
    }
    if let Some(latency) = contract.latency {
        context.interaction_level = match latency {
            LatencySensitivity::Critical | LatencySensitivity::High => {
                InteractionLevel::Interactive
            },
            LatencySensitivity::Medium => InteractionLevel::Passive,
            LatencySensitivity::Low => InteractionLevel::Idle,
        };
    }
    context
}

fn apply_task_hints(
    mut context: aicore_contracts::context::snapshot::ContextSnapshot,
    task: &aicore_contracts::task::queued::QueuedTask,
) -> aicore_contracts::context::snapshot::ContextSnapshot {
    use aicore_contracts::context::workload::InteractionLevel;
    use aicore_contracts::task::preferences::LatencySensitivity;

    if let Some(w) = task.contract.task {
        context.workload_type = w;
        context.confidence = 1.0;
    }
    if let Some(latency) = task.contract.latency {
        context.interaction_level = match latency {
            LatencySensitivity::Critical | LatencySensitivity::High => {
                InteractionLevel::Interactive
            },
            LatencySensitivity::Medium => InteractionLevel::Passive,
            LatencySensitivity::Low => InteractionLevel::Idle,
        };
    }
    context
}

fn derive_execution_targets(
    supplied: &ExecutionTargets,
    snapshot: &aicore_contracts::telemetry::snapshot::ComputerSnapshot,
) -> ExecutionTargets {
    if !supplied.foreground_pids.is_empty() || !supplied.background_pids.is_empty() {
        return supplied.clone();
    }
    let mut foreground_pids = Vec::new();
    let mut background_pids = Vec::new();
    for app in &snapshot.applications {
        if app.pid == 0 {
            continue;
        }
        if app.is_foreground {
            foreground_pids.push(app.pid);
        } else {
            background_pids.push(app.pid);
        }
    }
    for process in &snapshot.top_processes {
        if process.pid == 0 {
            continue;
        }
        if process.is_foreground {
            foreground_pids.push(process.pid);
        } else {
            background_pids.push(process.pid);
        }
    }
    foreground_pids.sort_unstable();
    foreground_pids.dedup();
    background_pids.retain(|pid| !foreground_pids.contains(pid));
    background_pids.sort_unstable();
    background_pids.dedup();
    ExecutionTargets {
        foreground_pids,
        background_pids,
        process_only: supplied.process_only,
    }
}

fn apply_task_contract_to_policy(
    policy: &mut ResourcePolicy,
    contract: &aicore_contracts::task::contract::TaskContract,
) {
    use aicore_contracts::policy::resource_policy::PowerMode;
    use aicore_contracts::task::preferences::{HardwareRequirement, PowerPreference};

    policy.task_id = contract.task_id.clone();
    if let Some(priority) = contract.priority {
        policy.priority = priority;
        policy.cpu.priority = priority;
        policy.gpu.priority = priority;
        policy.npu.priority = priority;
        policy.memory.priority = priority;
    }
    if let Some(power) = contract.power {
        policy.power.mode = match power {
            PowerPreference::PowerSave => PowerMode::PowerSave,
            PowerPreference::Balanced => PowerMode::Balanced,
            PowerPreference::Performance => PowerMode::Performance,
        };
    }
    if let Some(requirement) = contract.gpu_requirement {
        policy.gpu.workload_preference = match requirement {
            HardwareRequirement::NotNeeded => None,
            HardwareRequirement::Optional => Some("auto".to_string()),
            HardwareRequirement::Preferred | HardwareRequirement::Required => {
                Some("compute".to_string())
            },
        };
    }
    if let Some(requirement) = contract.npu_requirement {
        policy.npu.preferred = matches!(
            requirement,
            HardwareRequirement::Preferred | HardwareRequirement::Required
        );
    }
    if contract.privacy == Some(aicore_contracts::task::preferences::PrivacyRequirement::LocalOnly)
    {
        policy.reason.push_str(" privacy=local_only");
    }
}

/// Dashboard data field collection.
#[derive(Debug, Clone, Default)]
struct SnapshotFields {
    cpu_usage_percent: Option<f32>,
    gpu_usage_percent: Option<f32>,
    npu_usage_percent: Option<f32>,
    memory_usage_percent: Option<f32>,
    current_context_json: Option<String>,
    current_policy_json: Option<String>,
    runtime_route_json: Option<String>,
    ai_runtime_health_json: Option<String>,
}

#[cfg(test)]
mod record_id_tests {
    use super::historical_record_id;

    #[test]
    fn independent_loops_with_same_timestamp_and_tick_get_distinct_ids() {
        assert_ne!(
            historical_record_id(1_700_000_000_000, 1),
            historical_record_id(1_700_000_000_000, 1)
        );
    }
}
