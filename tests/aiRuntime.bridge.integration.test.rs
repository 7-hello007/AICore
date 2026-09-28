//! RuntimeBackedProvider bridge test.

use std::sync::Arc;

use aicore_ai_runtime::{PureRustRuntime, RuntimeBackedProvider};
use aicore_contracts::{
    ai::{
        input::AiInput,
        runtime::{ModelKind, ModelSource, ModelSpec},
        suggestion::AiSuggestionPayload,
    },
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::priority::{PolicyPriority, PolicySource},
    telemetry::snapshot::ComputerSnapshot,
    traits::{ai_provider::IAiProvider, ai_runtime::IAiRuntime},
};

const MODEL_JSON: &str = r#"{
    "schema_version": "1.0",
    "name": "policy_classifier_v1",
    "version": "1.0.0",
    "feature_dim": 4,
    "labels": ["boost_cpu", "boost_gpu", "boost_npu", "power_save", "keep_base"],
    "weights": [
        [0.9, 0.0, 0.0, 0.0],
        [0.0, 0.9, 0.0, 0.0],
        [0.0, 0.0, 0.9, 0.0],
        [0.0, 0.0, 0.0, 0.5],
        [0.2, 0.2, 0.2, 0.2]
    ]
}"#;

fn build_provider() -> RuntimeBackedProvider {
    let rt: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());
    let spec = ModelSpec {
        name: "policy_classifier_v1".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: MODEL_JSON.to_string(),
        },
        onnx_spec: None,
    };
    let handle = rt.load_model(&spec).unwrap();
    RuntimeBackedProvider::new(rt, handle, 4).with_min_confidence(0.3)
}

fn input_with(cpu: f32, gpu: f32, npu: f32, mem: f32) -> AiInput {
    let mut snap = ComputerSnapshot::default();
    snap.cpu.usage_percent = cpu;
    snap.gpu.usage_percent = Some(gpu);
    snap.npu.usage_percent = Some(npu);
    snap.memory.usage_percent = mem;

    AiInput {
        snapshot: snap,
        context: ContextSnapshot {
            workload_type: WorkloadType::Unknown,
            interaction_level: InteractionLevel::Interactive,
            ..Default::default()
        },
        base_policy: None,
        memory: None,
    }
}

#[test]
fn provider_loads_and_reports_available() {
    let p = build_provider();
    assert!(p.is_available());
    assert_eq!(p.name(), "runtime_backed_provider");
    assert!(p.capabilities().contains(&"policy.recommend"));
}

#[test]
fn high_cpu_suggests_boost_cpu() {
    let p = build_provider();
    let s = p.suggest(&input_with(80.0, 5.0, 0.0, 30.0)).unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            assert_eq!(policy.cpu.priority, PolicyPriority::High);
            assert_eq!(policy.source, PolicySource::AiRecommended);
        },
        _ => panic!("expected policy payload"),
    }
    assert!(s.reason.contains("runtime[pure_rust_runtime]"));
    assert!(s.reason.contains("boost_cpu"));
}

#[test]
fn high_npu_suggests_boost_npu() {
    let p = build_provider();
    let s = p.suggest(&input_with(10.0, 10.0, 90.0, 30.0)).unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(policy) => {
            assert_eq!(policy.npu.priority, PolicyPriority::High);
            assert!(policy.npu.preferred);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn low_everything_produces_valid_output() {
    // Low-resource scenario softmax distribution average, Top1 probability may be < 0.3.
    // Here's a loose threshold to verify that provider can output properly.
    let rt: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());
    let spec = ModelSpec {
        name: "policy_classifier_v1".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: MODEL_JSON.to_string(),
        },
        onnx_spec: None,
    };
    let handle = rt.load_model(&spec).unwrap();
    let p = RuntimeBackedProvider::new(rt, handle, 4).with_min_confidence(0.1);

    let s = p.suggest(&input_with(2.0, 2.0, 0.0, 5.0)).unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(_) => {},
        _ => panic!("expected policy payload"),
    }
    // Trust is legal
    assert!(s.confidence > 0.0 && s.confidence <= 1.0);
}

#[test]
fn low_everything_below_default_confidence_errors_gracefully() {
    // By default min_confidence = 0.3, the low resource scenario should return to Err.
    let p = build_provider();
    let result = p.suggest(&input_with(2.0, 2.0, 0.0, 5.0));
    assert!(result.is_err(), "expected low-confidence Err");
}

#[test]
fn policy_id_is_populated_when_base_empty() {
    let p = build_provider();
    let s = p.suggest(&input_with(80.0, 5.0, 0.0, 30.0)).unwrap();
    if let AiSuggestionPayload::Policy(policy) = s.payload {
        assert!(!policy.policy_id.is_empty());
        assert!(policy.created_at_unix_ms > 0);
    }
}

#[test]
fn provider_respects_base_policy() {
    use aicore_contracts::policy::resource_policy::ResourcePolicy;
    let p = build_provider();

    let mut input = input_with(80.0, 5.0, 0.0, 30.0);
    input.base_policy = Some(ResourcePolicy::default());

    let s = p.suggest(&input).unwrap();
    assert!(matches!(s.payload, AiSuggestionPayload::Policy(_)));
}
