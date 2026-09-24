//! 预测闭环集成测试。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::context::workload::{InteractionLevel, WorkloadType};
use aicore_contracts::execution::status::ExecutionOverallStatus as ExecStatus;
use aicore_contracts::policy::priority::PolicySource;
use aicore_contracts::policy::resource_policy::PowerMode;
use aicore_contracts::{
    application::id::ApplicationId,
    errors::ContractResult,
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    memory::{profile::UserProfile, record::HistoricalRecord},
    policy::resource_policy::ResourcePolicy,
    runtime::config::AdaptiveLoopConfig,
    task::{contract::TaskContract, queued::QueuedTask},
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        memory_store::IMemoryStore, policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, task_queue::ITaskQueue,
        telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::RuleBasedPolicyEngine,
    prediction::ShortHorizonPredictor,
};
use aicore_memory::InMemoryStore;
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
        s.collected_at_unix_ms = Some(14 * 3_600_000);
        Ok(s)
    }
    fn platform_name(&self) -> &'static str {
        "mock"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
}

#[test]
fn registered_task_uses_consented_application_pattern_and_records_identity() {
    let memory = Arc::new(InMemoryStore::new());
    let app = ApplicationId::new("app.editor");
    for day in 0..3 {
        let mut record = rec(
            &format!("app-{day}"),
            (day * 24 + 14) * 3_600_000,
            WorkloadType::Development,
        );
        record.application_id = Some(app.clone());
        memory.append_record(&record).unwrap();
    }
    for day in 3..6 {
        memory
            .append_record(&rec(
                &format!("system-{day}"),
                (day * 24 + 14) * 3_600_000,
                WorkloadType::Gaming,
            ))
            .unwrap();
    }
    let queue = Arc::new(InMemoryTaskQueue::new());
    queue
        .enqueue(QueuedTask::new(
            "task-app".into(),
            TaskContract::minimal(app.clone()),
            1,
        ))
        .unwrap();
    let loop_ = build_loop(memory.clone()).with_task_queue(queue);
    let outcome = loop_.tick(&ExecutionTargets::default());
    assert_eq!(outcome.predicted_workload, Some(WorkloadType::Development));
    assert!(outcome.validation_accepted);
    let history = memory.load_records(1).unwrap();
    assert_eq!(history[0].application_id, Some(app));
}

struct OkExecutor;

impl IPolicyExecutor for OkExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "test".into(),
                target: "system".into(),
                status: ExecutionStatus::Success,
                message: "ok".into(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "ok_executor"
    }
}

fn rec(id: &str, ts: u64, w: WorkloadType) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
        timestamp_unix_ms: ts,
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        policy_id: "p".to_string(),
        policy_source: PolicySource::RuleBased,
        overall_status: ExecStatus::AllSucceeded,
        cpu_usage_percent: 5.0,
        gpu_usage_percent: None,
        npu_usage_percent: None,
        memory_usage_percent: 30.0,
        power_mode: PowerMode::Balanced,
    }
}

fn build_loop(memory: Arc<InMemoryStore>) -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(OkExecutor);

    let loop_ = AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    );

    // 启用历史记录 + 预测器
    let profile = UserProfile {
        history_enabled: true,
        profile_enabled: true,
        ..Default::default()
    };
    memory.save_profile(&profile).unwrap();

    let memory_dyn: Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore> = memory;
    let predictor: Arc<dyn aicore_contracts::traits::predictor::IPredictor> =
        Arc::new(ShortHorizonPredictor::with_defaults());

    loop_.with_memory(memory_dyn).with_predictor(predictor)
}

#[test]
fn loop_without_memory_produces_no_prediction() {
    let memory = Arc::new(InMemoryStore::new());
    // 不启用 profile
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(OkExecutor);

    let loop_ = AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_predictor(Arc::new(ShortHorizonPredictor::with_defaults()));

    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(outcome.predicted_workload.is_none());
    assert!(outcome.prediction_confidence.is_none());
    let _ = memory;
}

#[test]
fn loop_with_consistent_history_predicts() {
    let memory = Arc::new(InMemoryStore::new());

    // 预置 5 条相同 workload 的记录
    for i in 0..5 {
        memory
            .append_record(&rec(
                &format!("r{}", i),
                1_000_000 + i,
                WorkloadType::AiInference,
            ))
            .unwrap();
    }

    let loop_ = build_loop(memory);
    let outcome = loop_.tick(&ExecutionTargets::default());

    // 预测应生效
    assert_eq!(
        outcome.predicted_workload,
        Some(WorkloadType::AiInference),
        "predicted={:?}",
        outcome.predicted_workload
    );
    assert!(outcome.prediction_confidence.unwrap() >= 0.5);
}

#[test]
fn loop_without_predictor_still_works() {
    let memory = Arc::new(InMemoryStore::new());
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(OkExecutor);

    let loop_ = AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    );

    let outcome = loop_.tick(&ExecutionTargets::default());
    assert!(outcome.success);
    assert!(outcome.predicted_workload.is_none());

    let _ = memory;
}

#[test]
fn prediction_does_not_break_safety_invariant() {
    let memory = Arc::new(InMemoryStore::new());
    for i in 0..5 {
        memory
            .append_record(&rec(
                &format!("r{}", i),
                1_000_000 + i,
                WorkloadType::Gaming,
            ))
            .unwrap();
    }

    let loop_ = build_loop(memory);
    for _ in 0..3 {
        let outcome = loop_.tick(&ExecutionTargets::default());
        // 无论预测结果如何，Validator 必须通过（policy_id 存在即代表通过）
        assert!(
            outcome.policy_id.is_some(),
            "policy must be generated even with prediction"
        );
        assert!(outcome.validation_accepted);
    }
}
