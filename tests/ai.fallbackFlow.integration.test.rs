//! AI fallback 集成：AI 可用时走 AI，不可用时走规则。

use std::sync::Arc;

use aicore_ai::{LocalFeatureProvider, MockAiProvider};
use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::priority::PolicySource,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        ai_provider::IAiProvider, policy_engine::IPolicyEngine, policy_validator::IPolicyValidator,
    },
};
use aicore_core::policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine};

fn ctx(w: WorkloadType) -> ContextSnapshot {
    ContextSnapshot {
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    }
}

#[test]
fn local_provider_drives_ai_assisted_engine() {
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let ai = LocalFeatureProvider::with_default_weights().unwrap();
    let engine = AiAssistedPolicyEngine::new(base, Some(Arc::new(ai)), 0.4);

    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(&snapshot, &ctx(WorkloadType::AiInference));

    // 无论 AI 命中与否，策略必须能通过 Validator
    let v = DefaultPolicyValidator::new();
    assert!(v.validate(&policy).accepted);
}

#[test]
fn unavailable_ai_falls_back_to_rule_based() {
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let ai: Arc<dyn IAiProvider> = Arc::new(LocalFeatureProvider::unavailable());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.4);

    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(&snapshot, &ctx(WorkloadType::Gaming));

    assert_eq!(policy.source, PolicySource::RuleBased);
}

#[test]
fn mock_ai_and_local_ai_both_valid() {
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let mock = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(mock), 0.4);

    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(&snapshot, &ctx(WorkloadType::Development));

    let v = DefaultPolicyValidator::new();
    assert!(v.validate(&policy).accepted);
}
