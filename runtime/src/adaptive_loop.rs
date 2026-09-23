use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

/// 顶层自适应循环。
///
/// 每次 tick 完成一次完整闭环：
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
            tick_counter: AtomicU64::new(0),
        }
    }

    /// 可选：启用记忆写入。写入前会检查用户 profile 是否允许记录。
    pub fn with_memory(mut self, memory: Arc<dyn IMemoryStore>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// 可选：启用短期预测器。
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

    /// 可选：启用透明执行器（六段式 + rollback）。
    pub fn with_transparent_executor(mut self, exec: Arc<dyn ITransparentExecutor>) -> Self {
        self.transparent = Some(exec);
        self
    }

    /// 可选：启用 Task 队列。
    pub fn with_task_queue(mut self, queue: Arc<dyn ITaskQueue>) -> Self {
        self.task_queue = Some(queue);
        self
    }

    /// 可选：启用 Runtime Router。
    pub fn with_runtime_router(mut self, router: Arc<dyn IRuntimeRouter>) -> Self {
        self.runtime_router = Some(router);
        self
    }

    /// 可选：启用 Runtime Binder。
    pub fn with_runtime_binder(mut self, binder: Arc<dyn IRuntimeBinder>) -> Self {
        self.runtime_binder = Some(binder);
        self
    }

    /// 注入独立 TierHub。Core 不知道 L0/L1/L2 的具体类型，只通过能力接口发现。
    pub fn with_tier_hub(mut self, hub: Arc<TierHub>) -> Self {
        self.tier_hub = Some(hub);
        self
    }

    /// 注入事件总线。事件总线仅负责发布观察结果，不参与业务决策。
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

    /// 从 memory store 构造 MemoryContext。
    ///
    /// - 无 memory：None。
    /// - profile 既不允许 history 也不允许 personalize：None（隐私优先）。
    /// - 有 memory：加载最近 N 条记录并聚合。
    fn build_memory_context(&self) -> Option<aicore_contracts::memory::context::MemoryContext> {
        let memory = self.memory.as_ref()?;

        aicore_memory::personalized_context(memory.as_ref(), 100)
            .ok()
            .flatten()
    }

    /// 应用短期预测。
    ///
    /// 返回 (merged_context, prediction)。
    /// - 无 predictor：返回原 context，prediction = None。
    /// - 预测失败或低置信度：返回原 context，prediction = None。
    /// - 预测成功：返回 workload_type 被替换的 context，prediction = Some。
    fn apply_prediction(
        &self,
        context: aicore_contracts::context::snapshot::ContextSnapshot,
        state: &aicore_core::domain::computer_state::ComputerState,
        memory_ctx: &Option<MemoryContext>,
    ) -> (
        aicore_contracts::context::snapshot::ContextSnapshot,
        Option<aicore_contracts::prediction::output::Prediction>,
    ) {
        let predictor = match &self.predictor {
            Some(p) => p,
            None => return (context, None),
        };

        let input = PredictionInput {
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

    /// 每 tick 最多取出一小批任务；队列实现仍负责原子 dequeue。
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

    /// 从 tick 收集 Dashboard 需要的字段。
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

    /// 从 runtime binder 生成 AI Runtime 健康摘要。
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

    /// 只做 Runtime 路由决策。该阶段无系统副作用。
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

    /// Policy 已通过 Validator 后才允许 Binder 执行。
    fn execute_runtime_binding(
        &self,
        route: Option<&aicore_contracts::runtime::route::RuntimeRoute>,
        state: &aicore_core::domain::computer_state::ComputerState,
        context: &aicore_contracts::context::snapshot::ContextSnapshot,
    ) -> Option<BindingResult> {
        let route = route?;
        let binder = self.runtime_binder.as_ref()?;
        let inference = if matches!(
            route.source,
            aicore_contracts::runtime::route::RuntimeRouteSource::AiRuntime
                | aicore_contracts::runtime::route::RuntimeRouteSource::Cpu
        ) {
            Some(aicore_contracts::ai::runtime::InferenceRequest {
                features: vec![
                    (state.snapshot.cpu.usage_percent / 100.0).clamp(0.0, 1.0),
                    (state.snapshot.gpu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
                    (state.snapshot.npu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
                    (state.snapshot.memory.usage_percent / 100.0).clamp(0.0, 1.0),
                ],
                tensors: Vec::new(),
            })
        } else {
            None
        };
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

impl IRuntimeOrchestrator for AdaptiveLoop {
    fn tick(&self, targets: &ExecutionTargets) -> TickOutcome {
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

                return TickOutcome {
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
                };
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

        let memory_ctx = self.build_memory_context();
        let (mut context, prediction) = self.apply_prediction(context, &state, &memory_ctx);

        // 2.5：L1 显式 task contract 优先于 L0 推断；L2 provider 独立注册。
        let tier_task = self.tier_hub.as_ref().and_then(|hub| {
            hub.best_task_source()
                .ok()
                .flatten()
                .and_then(|src| src.current_task().ok().flatten())
        });
        // 一个 tick 只执行一个 task，避免一个 policy 被错误地用于多个任务后又全部标记完成。
        const MAX_TASKS_PER_TICK: usize = 1;
        let active_tasks = self.dequeue_tasks(MAX_TASKS_PER_TICK);
        let active_task = active_tasks.first();
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
        let mut runtime_binding: Option<BindingResult> = None;
        tracing::debug!(
            component = "adaptive_loop",
            event = "runtime_routing",
            trace_id = %trace.trace_id,
            tick_id = tick_id,
            route_source = ?runtime_route.as_ref().map(|r| r.source.as_str()),
            binding_executed = ?runtime_binding.as_ref().map(|b| b.executed),
            "runtime routing complete"
        );

        // 3. Decide：根据 mode 选择引擎
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

        // TaskContract 是显式应用约束，必须在任何 Tier policy override 之后重新施加。
        if let Some(contract) = tier_task
            .as_ref()
            .or_else(|| active_task.map(|task| &task.contract))
        {
            apply_task_contract_to_policy(&mut policy, contract);
        }

        if let Some(guard) = &self.thermal_guard {
            guard.constrain(&state.snapshot, &mut policy);
        }

        // V4：收集 Dashboard 数据面字段
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
                for task in &active_tasks {
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

            return TickOutcome {
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
            };
        }

        // Policy 已通过 Validator；此后才允许 Runtime Binder 产生推理/硬件副作用。
        runtime_binding = self.execute_runtime_binding(runtime_route.as_ref(), &state, &context);

        // Platform mode 可从本 tick 的真实 telemetry 推导 process targets；显式 targets 始终优先。
        let effective_targets = derive_execution_targets(targets, &state.snapshot);

        // 5. Execute（透明执行器优先）
        let prior = self.last_applied_policy.lock().ok().and_then(|g| g.clone());

        let (exec_result, control_trace) = if let Some(tx) = &self.transparent {
            let trace = tx.execute_transparent(&policy, &effective_targets, prior.as_ref());
            // 保留 Apply 的明细，并使用 Verify / Rollback 后的最终状态。
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
            // 若 rollback 触发且成功，则视为 NoOp，避免 SafeMode 误判
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

        // 更新 last_applied_policy（仅在成功时）
        if exec_success {
            if let Ok(mut g) = self.last_applied_policy.lock() {
                *g = Some(policy.clone());
            }
        }

        // 6. Measure
        let duration_ms = start.elapsed().as_millis() as u64;
        self.metrics.record_tick(exec_success, duration_ms);

        // 7. SafeMode 转换
        // - 成功 → record_success（重置失败计数）
        // - 真失败 → record_failure（累加失败计数）
        // - NoOp（能力缺失 / 全部跳过）→ 不改变 SafeMode，避免误触发
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
        if matches!(overall, ExecutionOverallStatus::NoOp) {
            self.metrics.record_noop_tick();
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

        // 7.5 Task：本 tick 消费到的全部任务都完成/失败，避免队列长期堆积。
        if let Some(queue) = &self.task_queue {
            for task in &active_tasks {
                let result = if exec_success {
                    queue.complete(&task.task_id, Some(format!("{:?}", overall)))
                } else if exec_failure {
                    queue.fail(&task.task_id, format!("{:?}", overall))
                } else {
                    // 任务没有实际执行，不能把 NoOp 记为 Completed。
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

        // 8. Learn：写入历史（若已启用）
        let mut recorded = false;
        if let Some(memory) = &self.memory {
            if Self::should_record_history(memory) {
                let timestamp_unix_ms = Self::now_unix_ms();
                let record = HistoricalRecord {
                    record_id: historical_record_id(timestamp_unix_ms, tick_id),
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
                if memory.append_record(&record).is_ok() {
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
    /// 便捷构造：使用默认 validator。
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

/// 用 Task hints 覆盖 context。
///
/// 语义：
/// - `contract.task` 覆盖 workload_type。
/// - `latency` 影响 interaction_level。
/// - 只覆盖已声明的字段；未声明的保留原 context。
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

/// Dashboard 数据面字段集合。
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
