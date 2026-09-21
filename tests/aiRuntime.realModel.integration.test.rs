//! OnnxRuntimeBackend + RuntimeBackedProvider 集成。

use std::path::PathBuf;
use std::sync::Arc;

use aicore_ai_runtime::{MockOnnxSessionFactory, OnnxRuntimeBackend, RuntimeBackedProvider};
use aicore_contracts::{
    ai::{
        input::AiInput,
        runtime::{ModelKind, ModelSource, ModelSpec, OnnxModelSpec, TensorDType, TensorSchema},
        suggestion::AiSuggestionPayload,
    },
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::priority::PolicySource,
    telemetry::snapshot::ComputerSnapshot,
    traits::{ai_provider::IAiProvider, ai_runtime::IAiRuntime},
};

fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("models/policy_classifier_v1.json")
}

fn spec() -> ModelSpec {
    ModelSpec {
        name: "onnx_inference_test".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::Onnx,
        source: ModelSource::File {
            path: model_path().to_string_lossy().to_string(),
        },
        onnx_spec: Some(OnnxModelSpec {
            input_schema: vec![TensorSchema {
                name: "input".to_string(),
                shape: vec![1, 4],
                dtype: TensorDType::F32,
            }],
            output_schema: vec![],
            preferred_provider: None,
        }),
    }
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
fn full_lifecycle_load_infer_unload() {
    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    let backend: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(factory));

    // load
    let handle = backend.load_model(&spec()).unwrap();

    // infer
    let req = aicore_contracts::ai::runtime::InferenceRequest {
        features: vec![],
        tensors: vec![aicore_contracts::ai::runtime::TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![0.9, 0.0, 0.0, 0.0],
        }],
    };
    let resp = backend.infer(&handle, &req).unwrap();
    assert!(!resp.labels.is_empty());

    // unload
    backend.unload(&handle).unwrap();
}

#[test]
fn onnx_backend_not_used_by_runtime_backed_provider_with_feature_tensors() {
    // RuntimeBackedProvider 目前用 features 路径（V3-F 的 FeatureWeighted 输入）。
    // ONNX backend 需要 tensors 路径；此处只验证"若 provider 发 features-only 请求，
    // ONNX backend 会拒绝"，证明 provider 与 backend 的输入契约边界清晰。
    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    let backend: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(factory));

    let handle = backend.load_model(&spec()).unwrap();
    let req = aicore_contracts::ai::runtime::InferenceRequest {
        features: vec![0.9, 0.0, 0.0, 0.0],
        tensors: vec![],
    };
    let err = backend.infer(&handle, &req).unwrap_err();
    assert!(format!("{}", err).contains("tensors"));

    let _provider = RuntimeBackedProvider::new(backend, handle, 4);
    let _ = input_with(80.0, 5.0, 0.0, 30.0);
}

#[test]
fn provider_still_works_with_pure_rust() {
    use aicore_ai_runtime::PureRustRuntime;

    let rt: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());
    // FeatureWeighted 模型
    let json = r#"{
        "schema_version": "1.0",
        "name": "test",
        "version": "1.0.0",
        "feature_dim": 4,
        "labels": ["boost_cpu", "boost_gpu"],
        "weights": [[0.9, 0.0, 0.0, 0.0], [0.0, 0.9, 0.0, 0.0]]
    }"#;
    let spec = ModelSpec {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: json.to_string(),
        },
        onnx_spec: None,
    };
    let handle = rt.load_model(&spec).unwrap();
    let provider = RuntimeBackedProvider::new(rt, handle, 4).with_min_confidence(0.1);

    let s = provider.suggest(&input_with(80.0, 5.0, 0.0, 30.0)).unwrap();
    match s.payload {
        AiSuggestionPayload::Policy(p) => {
            assert_eq!(p.source, PolicySource::AiRecommended);
        },
        _ => panic!("expected policy"),
    }
}

#[test]
fn spec_serde_roundtrip_with_onnx() {
    let s = spec();
    let json = serde_json::to_string(&s).unwrap();
    let back: ModelSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}
