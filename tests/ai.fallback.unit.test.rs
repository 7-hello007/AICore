use std::sync::Arc;

use aicore_ai::MockAiProvider;
use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::{priority::PolicySource, resource_policy::ResourcePolicy},
    telemetry::snapshot::ComputerSnapshot,
    traits::policy_engine::IPolicyEngine,
};
use aicore_core::policy::{AiAssistedPolicyEngine, RuleBasedPolicyEngine};

fn ctx(workload: WorkloadType) -> ContextSnapshot {
    ContextSnapshot {
        workload_type: workload,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    }
}

#[test]
fn ai_unavailable_falls_back_to_rule_based() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::unavailable());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.6);

    let snapshot = ComputerSnapshot::default();
    let (policy, suggestion) = engine.generate_with_meta(&snapshot, &ctx(WorkloadType::Gaming));

    assert_eq!(policy.source, PolicySource::RuleBased);
    assert!(suggestion.is_none());
}

#[test]
fn low_confidence_suggestion_keeps_rule_based_policy() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.95);

    // Development 场景 Mock 只给 0.5 置信度，低于阈值 0.95 → 走规则
    let snapshot = ComputerSnapshot::default();
    let (policy, suggestion) =
        engine.generate_with_meta(&snapshot, &ctx(WorkloadType::Development));

    assert_eq!(policy.source, PolicySource::RuleBased);
    assert!(suggestion.is_some());
    let s = suggestion.unwrap();
    assert!(s.confidence < 0.95);
}

#[test]
fn high_confidence_suggestion_is_used() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.6);

    let mut snapshot = ComputerSnapshot::default();
    snapshot.npu.available = true;

    let (policy, suggestion) =
        engine.generate_with_meta(&snapshot, &ctx(WorkloadType::AiInference));

    assert_eq!(policy.source, PolicySource::AiRecommended);
    assert!(suggestion.is_some());
}

#[test]
fn no_ai_registered_keeps_rule_based() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let engine = AiAssistedPolicyEngine::new(base, None, 0.6);

    let snapshot = ComputerSnapshot::default();
    let (policy, suggestion) = engine.generate_with_meta(&snapshot, &ctx(WorkloadType::Gaming));

    assert_eq!(policy.source, PolicySource::RuleBased);
    assert!(suggestion.is_none());
}

#[test]
fn engine_reports_names() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.6);

    assert_eq!(engine.name(), "ai_assisted_policy_engine");
    assert_eq!(engine.base_name(), "rule_based_policy_engine");
    assert_eq!(engine.ai_name(), Some("mock_ai_provider"));
    assert!((engine.confidence_threshold() - 0.6).abs() < f32::EPSILON);
}

/// 防止空 ResourcePolicy 时 panic。
#[test]
fn empty_input_does_not_panic() {
    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.0);

    let snapshot = ComputerSnapshot::default();
    let _: ResourcePolicy = engine.generate(&snapshot, &ContextSnapshot::default());
}
