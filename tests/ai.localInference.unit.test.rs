//! Local AI provider inference tests.

use aicore_ai::{
    local::{FeatureVector, PolicyIntent, WeightTable},
    LocalFeatureProvider,
};
use aicore_contracts::{
    ai::{
        input::AiInput,
        model_metadata::{ModelRuntime, PrivacyClass},
        suggestion::AiSuggestionPayload,
    },
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::CpuPolicy,
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::ai_provider::IAiProvider,
};

fn input_with(cpu: f32, gpu: f32, npu: f32, interaction: InteractionLevel) -> AiInput {
    let mut snapshot = ComputerSnapshot::default();
    snapshot.cpu.usage_percent = cpu;
    snapshot.gpu.usage_percent = Some(gpu);
    snapshot.npu.usage_percent = Some(npu);
    snapshot.memory.usage_percent = 50.0;

    let context = ContextSnapshot {
        workload_type: WorkloadType::Unknown,
        interaction_level: interaction,
        ..Default::default()
    };

    AiInput {
        snapshot,
        context,
        base_policy: None,
        memory: None,
    }
}

#[test]
fn default_weights_validate() {
    let w = WeightTable::default_weights();
    assert!(w.validate().is_ok());
}

#[test]
fn feature_vector_is_normalized() {
    let input = input_with(150.0, 200.0, -10.0, InteractionLevel::Interactive);
    let fv = FeatureVector::from_input(&input);
    for v in fv.values.iter() {
        assert!((0.0..=1.0).contains(v), "value {} out of range", v);
    }
}

#[test]
fn provider_reports_metadata() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let m = p.metadata();
    assert_eq!(m.runtime, ModelRuntime::NativeRust);
    assert_eq!(m.privacy_class, PrivacyClass::LocalOnly);
    assert!(m.is_local_only());
    assert!(m.latency_budget_ms > 0);
    assert!(m.validate().is_ok());
}

#[test]
fn provider_is_available_by_default() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    assert!(p.is_available());
}

#[test]
fn provider_is_unavailable_when_constructed_so() {
    let p = LocalFeatureProvider::unavailable();
    assert!(!p.is_available());
}

#[test]
fn provider_produces_policy_suggestion() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let s = p
        .suggest(&input_with(80.0, 10.0, 0.0, InteractionLevel::Interactive))
        .unwrap();
    assert!(s.confidence > 0.0 && s.confidence <= 1.0);
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            assert_eq!(policy.source, PolicySource::AiRecommended);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn provider_output_is_deterministic() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let input = input_with(80.0, 10.0, 0.0, InteractionLevel::Interactive);
    let s1 = p.suggest(&input).unwrap();
    let s2 = p.suggest(&input).unwrap();

    // Other fields should be consistent except suggestion_id and time stamp
    assert_eq!(s1.confidence, s2.confidence);
    assert_eq!(s1.reason, s2.reason);
}

#[test]
fn high_npu_prefers_boost_npu() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let s = p
        .suggest(&input_with(10.0, 10.0, 90.0, InteractionLevel::Interactive))
        .unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            // NPU priority should be raised at high NPU
            assert_eq!(policy.npu.priority, PolicyPriority::High);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn policy_invariant_holds() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let s = p
        .suggest(&input_with(80.0, 80.0, 80.0, InteractionLevel::Interactive))
        .unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            let max_sub = [
                policy.cpu.priority,
                policy.gpu.priority,
                policy.npu.priority,
                policy.memory.priority,
            ]
            .into_iter()
            .max()
            .unwrap();
            assert!(policy.priority >= max_sub);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn weights_roundtrip_json() {
    let w = WeightTable::default_weights();
    let json = w.to_json().unwrap();
    let back = WeightTable::from_json(&json).unwrap();
    assert_eq!(back, w);
}

#[test]
fn invalid_weights_are_rejected() {
    let mut w = WeightTable::default_weights();
    // Clear an intent
    w.entries.retain(|e| e.intent != PolicyIntent::BoostCpu);
    assert!(w.validate().is_err());
}

#[test]
fn custom_base_policy_is_respected() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let mut input = input_with(30.0, 30.0, 30.0, InteractionLevel::Passive);

    let base = aicore_contracts::policy::resource_policy::ResourcePolicy {
        cpu: CpuPolicy {
            priority: PolicyPriority::High,
            affinity_cores: None,
        },
        priority: PolicyPriority::High,
        ..Default::default()
    };
    input.base_policy = Some(base);

    let s = p.suggest(&input).unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            // KeepBase Time CPU High priority should be retained
            assert_eq!(policy.cpu.priority, PolicyPriority::High);
        },
        _ => panic!("expected policy payload"),
    }
}
