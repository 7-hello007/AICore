use aicore_contracts::{
    execution::{status::ExecutionOverallStatus, targets::ExecutionTargets},
    policy::resource_policy::ResourcePolicy,
    traits::{
        policy_executor::IPolicyExecutor,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
    },
};

use aicore_core::model::{
    ModelEnvironment, ModelPolicyExecutor, ModelPostConditionChecker, ModelTargetProvider,
    MODEL_BACKGROUND_PID, MODEL_FOREGROUND_PID,
};

#[test]
fn model_target_provider_returns_virtual_targets() {
    let provider = ModelTargetProvider::default();
    let targets = provider.execution_targets();

    assert_eq!(targets.foreground_pids, vec![MODEL_FOREGROUND_PID]);

    assert_eq!(targets.background_pids, vec![MODEL_BACKGROUND_PID]);
}

#[test]
fn model_executor_reports_model_success_without_host_execution() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-test-1".to_string(),
        ..Default::default()
    };

    let targets = ModelTargetProvider::default().execution_targets();

    let result = executor.execute(&policy, &targets);

    assert_eq!(result.overall_status, ExecutionOverallStatus::AllSucceeded);

    assert!(!result.outcomes.is_empty());

    assert!(result.outcomes.iter().all(|outcome| {
        matches!(
            outcome.status,
            aicore_contracts::execution::status::ExecutionStatus::Success
        )
    }));

    let state = environment.snapshot();

    assert_eq!(state.last_policy_id.as_deref(), Some("model-test-1"));

    assert_eq!(
        state.processes.get(&MODEL_FOREGROUND_PID).unwrap().priority,
        policy.cpu.priority
    );
}

#[test]
fn empty_targets_are_replaced_by_model_targets() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-empty-target-test".to_string(),
        ..Default::default()
    };
    let result = executor.execute(&policy, &ExecutionTargets::default());

    assert_eq!(result.overall_status, ExecutionOverallStatus::AllSucceeded);

    assert!(result
        .outcomes
        .iter()
        .any(|outcome| outcome.resource == "model.process.priority"));
}

#[test]
fn model_post_condition_passes_after_apply() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-verify-test".to_string(),
        ..Default::default()
    };

    let targets = ModelTargetProvider::default().execution_targets();

    let result = executor.execute(&policy, &targets);

    let checker = ModelPostConditionChecker::new(environment);

    assert!(matches!(
        checker.verify(&policy, &result),
        PostConditionResult::Passed
    ));
}

#[test]
fn model_pipeline_runs_shared_json_runtime_and_completes_task() {
    use aicore_ai_runtime::{PureRustRuntime, RuntimeBackedProvider};
    use aicore_contracts::{
        ai::runtime::{ModelKind, ModelSource, ModelSpec},
        application::id::ApplicationId,
        context::workload::WorkloadType,
        errors::ContractResult,
        memory::profile::UserProfile,
        runtime::config::AdaptiveLoopConfig,
        task::{contract::TaskContract, lifecycle::TaskState, queued::QueuedTask},
        telemetry::snapshot::ComputerSnapshot,
        traits::{
            ai_runtime::IAiRuntime, memory_store::IMemoryStore,
            runtime_orchestrator::IRuntimeOrchestrator, task_queue::ITaskQueue,
            telemetry_provider::ITelemetryProvider,
        },
    };
    use aicore_core::{
        context::{ContextEngine, RuleBasedClassifier},
        event_bus::{Event, EventBus, EventSubscriber},
        executor::TransparentExecutor,
        policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine},
        prediction::ShortHorizonPredictor,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Telemetry;
    impl ITelemetryProvider for Telemetry {
        fn collect(&self) -> ContractResult<ComputerSnapshot> {
            let mut snapshot = ComputerSnapshot::default();
            snapshot.cpu.logical_cores = 4;
            snapshot.cpu.usage_percent = 20.0;
            Ok(snapshot)
        }
        fn platform_name(&self) -> &'static str {
            "model"
        }
        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }
    }
    struct Events(AtomicUsize);
    impl EventSubscriber for Events {
        fn handle(&self, _: &Event) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn name(&self) -> &'static str {
            "model-events"
        }
    }
    let runtime = Arc::new(PureRustRuntime::new());
    let handle = runtime
        .load_model(&ModelSpec {
            name: "policy_classifier_v1".into(),
            version: "1.0.0".into(),
            kind: ModelKind::FeatureWeighted,
            source: ModelSource::Inline {
                json: include_str!("../models/policy_classifier_v1.json").into(),
            },
            onnx_spec: None,
        })
        .unwrap();
    let provider = Arc::new(RuntimeBackedProvider::new(
        runtime.clone(),
        handle.clone(),
        4,
    ));
    let binder = Arc::new(
        aicore_runtime::ModelRuntimeBinder::new().with_ai_runtime(runtime.clone(), handle),
    );
    let environment = ModelEnvironment::new();
    let executor = Arc::new(ModelPolicyExecutor::new(environment.clone()));
    let transparent = Arc::new(
        TransparentExecutor::new(executor.clone()).with_checker(Arc::new(
            ModelPostConditionChecker::new(environment.clone()),
        )),
    );
    let memory = Arc::new(aicore_memory::InMemoryStore::new());
    memory
        .save_profile(&UserProfile {
            history_enabled: true,
            ..Default::default()
        })
        .unwrap();
    let queue = Arc::new(aicore_task_runtime::InMemoryTaskQueue::new());
    let mut contract = TaskContract::minimal(ApplicationId::new("model.pipeline"));
    contract.task_id = Some("compile-task".into());
    contract.task = Some(WorkloadType::Compilation);
    queue
        .enqueue(QueuedTask::new("compile-task".into(), contract, 0))
        .unwrap();
    let hub = aicore_tier::TierHub::new();
    aicore_tier_l0::register_l0(&hub).unwrap();
    aicore_tier_l1::register_l1(&hub).unwrap();
    aicore_tier_l2::register_l2(&hub).unwrap();
    let events = Arc::new(Events(AtomicUsize::new(0)));
    let bus = Arc::new(EventBus::new());
    bus.subscribe(events.clone());
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let loop_ = aicore_runtime::AdaptiveLoop::new(
        Arc::new(Telemetry),
        Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
        Arc::new(AiAssistedPolicyEngine::new(
            base.clone(),
            Some(provider),
            0.6,
        )),
        base,
        Arc::new(DefaultPolicyValidator::new()),
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_memory(memory.clone())
    .with_task_queue(queue.clone())
    .with_tier_hub(Arc::new(hub))
    .with_event_bus(bus)
    .with_predictor(Arc::new(ShortHorizonPredictor::with_defaults()))
    .with_runtime_router(Arc::new(aicore_runtime_router::SafeRuntimeRouter::new()))
    .with_runtime_binder(binder)
    .with_transparent_executor(transparent);
    let outcome = loop_.tick(&ModelTargetProvider::default().execution_targets());
    assert!(outcome.success, "{}", outcome.message);
    assert_eq!(outcome.runtime_route_source.as_deref(), Some("cpu"));
    assert_eq!(outcome.runtime_binding_executed, Some(true));
    assert_eq!(loop_.metrics().ai_inference_count, 1);
    assert_eq!(runtime.loaded_model_count(), 1);
    assert_eq!(
        queue.get("compile-task").unwrap().unwrap().state,
        TaskState::Completed
    );
    assert_eq!(memory.record_count().unwrap(), 1);
    assert!(events.0.load(Ordering::SeqCst) >= 4);
    assert!(outcome.current_context_json.is_some());
    assert!(outcome.current_policy_json.is_some());
    let health: serde_json::Value =
        serde_json::from_str(&outcome.ai_runtime_health_json.unwrap()).unwrap();
    assert_eq!(health["loaded_models"], 1);
    assert_eq!(environment.snapshot().processes.len(), 2);
}
