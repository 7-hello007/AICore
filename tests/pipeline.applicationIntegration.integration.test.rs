use std::sync::Arc;

use aicore_api::{ApplicationRegistry, PublicApi};
use aicore_contracts::{
    application::id::ApplicationId,
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::priority::PolicyPriority,
    task::{contract::TaskContract, preferences::LatencySensitivity},
    telemetry::snapshot::ComputerSnapshot,
    traits::{policy_engine::IPolicyEngine, policy_validator::IPolicyValidator},
};
use aicore_core::policy::{DefaultPolicyValidator, RuleBasedPolicyEngine};

fn setup() -> (Arc<ApplicationRegistry>, PublicApi, ApplicationId) {
    let reg = Arc::new(ApplicationRegistry::new());
    let app = ApplicationId::new("com.example.ai-app");
    reg.register(
        app.clone(),
        "AI App",
        "1.0",
        vec!["npu.inference".to_string()],
    )
    .unwrap();
    let api = PublicApi::new(reg.clone());
    (reg, api, app)
}

#[test]
fn contract_overrides_workload() {
    let (_reg, api, app) = setup();

    let base = ContextSnapshot {
        workload_type: WorkloadType::GeneralOffice,
        interaction_level: InteractionLevel::Passive,
        confidence: 0.4,
        ..Default::default()
    };

    let contract = TaskContract {
        task: Some(WorkloadType::AiInference),
        ..TaskContract::minimal(app)
    };

    let merged = api.merge_context(&base, &contract).unwrap();
    assert_eq!(merged.workload_type, WorkloadType::AiInference);
    // Apply explicit declaration → confidence = 1.0
    assert!((merged.confidence - 1.0).abs() < f32::EPSILON);
}

#[test]
fn contract_overrides_interaction_via_latency() {
    let (_reg, api, app) = setup();

    let base = ContextSnapshot {
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Idle,
        confidence: 0.3,
        ..Default::default()
    };

    let contract = TaskContract {
        latency: Some(LatencySensitivity::Critical),
        ..TaskContract::minimal(app)
    };

    let merged = api.merge_context(&base, &contract).unwrap();
    assert_eq!(merged.interaction_level, InteractionLevel::Interactive);
}

#[test]
fn contract_without_task_keeps_base_workload() {
    let (_reg, api, app) = setup();

    let base = ContextSnapshot {
        workload_type: WorkloadType::Gaming,
        interaction_level: InteractionLevel::Interactive,
        confidence: 0.85,
        ..Default::default()
    };

    let contract = TaskContract::minimal(app);
    let merged = api.merge_context(&base, &contract).unwrap();

    assert_eq!(merged.workload_type, WorkloadType::Gaming);
    assert!((merged.confidence - 0.85).abs() < f32::EPSILON);
}

#[test]
fn unregistered_app_merge_is_rejected() {
    let reg = Arc::new(ApplicationRegistry::new());
    let api = PublicApi::new(reg);

    let base = ContextSnapshot::default();
    let contract = TaskContract::minimal(ApplicationId::new("com.example.unknown"));
    let err = api.merge_context(&base, &contract).unwrap_err();
    assert!(format!("{}", err).contains("ERR_APP_NOT_REGISTERED"));
}

#[test]
fn full_pipeline_contract_to_validated_policy() {
    let (_reg, api, app) = setup();

    // 1. Applications for registration and submission of contracts
    let contract = TaskContract {
        task: Some(WorkloadType::AiInference),
        priority: Some(PolicyPriority::High),
        latency: Some(LatencySensitivity::High),
        ..TaskContract::minimal(app)
    };
    let result = api.submit_task(&contract).unwrap();
    assert!(result.accepted);

    // Baseline context for ContextEngine (simulation)
    let base = ContextSnapshot {
        workload_type: WorkloadType::GeneralOffice,
        interaction_level: InteractionLevel::Passive,
        confidence: 0.5,
        ..Default::default()
    };

    // Consolidation compacts
    let merged = api.merge_context(&base, &contract).unwrap();
    assert_eq!(merged.workload_type, WorkloadType::AiInference);
    assert_eq!(merged.interaction_level, InteractionLevel::Interactive);

    // 4. PolicyEngine
    let engine = RuleBasedPolicyEngine::new();
    let policy = engine.generate(&ComputerSnapshot::default(), &merged);
    assert_eq!(
        policy.source,
        aicore_contracts::policy::priority::PolicySource::RuleBased
    );

    // 5. Validator
    let validator = DefaultPolicyValidator::new();
    let r = validator.validate(&policy);
    assert!(r.accepted, "policy rejected: {:?}", r.issues);
}

#[test]
fn api_registry_is_shared_with_callers() {
    let (reg, api, app) = setup();
    // PublicApi::registry() share with external Arc
    assert!(Arc::ptr_eq(api.registry(), &reg));
    assert!(api.registry().is_registered(&app).unwrap());
}
