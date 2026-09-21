use std::sync::Arc;

use aicore_ai::{InferenceRouter, MockAiProvider};
use aicore_contracts::{
    ai::{
        input::AiInput,
        suggestion::{AiSource, AiSuggestionPayload},
        task::AiTaskType,
    },
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::ai_provider::IAiProvider,
};

fn make_input(workload: WorkloadType, npu_available: bool) -> AiInput {
    let mut snapshot = ComputerSnapshot::default();
    snapshot.npu.available = npu_available;

    let context = ContextSnapshot {
        workload_type: workload,
        interaction_level: InteractionLevel::Interactive,
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
fn mock_provider_is_available() {
    let p = MockAiProvider::new();
    assert!(p.is_available());
    assert_eq!(p.name(), "mock_ai_provider");
    assert!(p.capabilities().contains(&"policy.recommend"));
}

#[test]
fn mock_provider_suggests_npu_policy_on_ai_inference() {
    let p = MockAiProvider::new();
    let s = p
        .suggest(&make_input(WorkloadType::AiInference, true))
        .expect("suggest");

    assert_eq!(s.task, AiTaskType::RecommendPolicy);
    assert_eq!(s.source, AiSource::Mock);
    assert!(s.confidence >= 0.8);

    match s.payload {
        AiSuggestionPayload::Policy(p) => {
            assert!(p.npu.preferred);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn mock_provider_falls_back_when_npu_unavailable() {
    let p = MockAiProvider::new();
    let s = p
        .suggest(&make_input(WorkloadType::AiInference, false))
        .expect("suggest");
    // 没有 NPU 时返回低置信度
    assert!(s.confidence < 0.5);
}

#[test]
fn router_returns_first_available_provider() {
    let mut router = InferenceRouter::new();
    router.register(Arc::new(MockAiProvider::unavailable()));
    router.register(Arc::new(MockAiProvider::new()));
    assert_eq!(router.provider_count(), 2);

    let s = router
        .suggest(&make_input(WorkloadType::AiInference, true))
        .expect("suggest");
    assert_eq!(s.source, AiSource::Mock);
}

#[test]
fn router_without_providers_errors() {
    let router = InferenceRouter::new();
    let err = router.suggest(&make_input(WorkloadType::AiInference, true));
    assert!(err.is_err());
}

#[test]
fn router_all_unavailable_errors() {
    let mut router = InferenceRouter::new();
    router.register(Arc::new(MockAiProvider::unavailable()));
    let err = router.suggest(&make_input(WorkloadType::AiInference, true));
    assert!(err.is_err());
}
