//! OnnxRuntimeBackend unit test (based on MockOnnxSession).

use std::path::PathBuf;
use std::sync::Arc;

use aicore_ai_runtime::{MockOnnxSessionFactory, OnnxRuntimeBackend, PureRustRuntime};
use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, ModelKind, ModelSource, ModelSpec, OnnxModelSpec, RuntimeCapability,
        RuntimeHealth, TensorDType, TensorInput, TensorSchema,
    },
    errors::{AicoreError, ContractError},
    traits::ai_runtime::IAiRuntime,
};

fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("models/policy_classifier_v1.json")
}

fn default_spec() -> ModelSpec {
    ModelSpec {
        name: "onnx_test".to_string(),
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
            output_schema: vec![TensorSchema {
                name: "output".to_string(),
                shape: vec![1, 4],
                dtype: TensorDType::F32,
            }],
            preferred_provider: None,
        }),
    }
}

fn make_backend() -> OnnxRuntimeBackend {
    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    OnnxRuntimeBackend::new(factory)
}

#[test]
fn backend_reports_name_and_capabilities() {
    let backend = make_backend();
    assert_eq!(backend.runtime_name(), "onnx_runtime_backend");

    let caps = backend.capabilities();
    assert!(caps.contains(&RuntimeCapability::LoadFromFile));
    assert!(caps.contains(&RuntimeCapability::OnnxInference));
    assert!(!caps.contains(&RuntimeCapability::FeatureInference));
}

#[test]
fn backend_health_starts_healthy_zero_models() {
    let backend = make_backend();
    match backend.health() {
        RuntimeHealth::Healthy { loaded_models: 0 } => {},
        other => panic!("unexpected health: {:?}", other),
    }
}

#[test]
fn load_onnx_model_succeeds() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();
    assert_eq!(handle.model_name, "onnx_test");
    assert_eq!(handle.model_version, "1.0.0");
    assert!(handle.handle_id.starts_with("onnx-"));
    assert_eq!(backend.loaded_count(), 1);
}

#[test]
fn load_feature_weighted_model_rejected() {
    let backend = make_backend();
    let spec = ModelSpec {
        kind: ModelKind::FeatureWeighted,
        onnx_spec: None,
        ..default_spec()
    };
    let err = backend.load_model(&spec).unwrap_err();
    assert!(matches!(err, ContractError::NotSupported(_)));
}

#[test]
fn load_missing_onnx_spec_rejected() {
    let backend = make_backend();
    let mut spec = default_spec();
    spec.onnx_spec = None;
    let err = backend.load_model(&spec).unwrap_err();
    // Validate will fail first.
    assert!(matches!(err, AicoreError::ContractInvalid(_)));
}

#[test]
fn infer_with_valid_tensor_succeeds() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();

    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![1.0, 0.5, 0.0, 0.0],
        }],
    };
    let resp = backend.infer(&handle, &req).unwrap();
    assert_eq!(resp.labels.len(), 4);
    assert_eq!(resp.model_version, "1.0.0");

    let total: f32 = resp.labels.iter().map(|l| l.score).sum();
    assert!((total - 1.0).abs() < 0.001);
}

#[test]
fn infer_without_tensors_rejected() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();
    let req = InferenceRequest {
        features: vec![1.0, 2.0],
        tensors: vec![],
    };
    let err = backend.infer(&handle, &req).unwrap_err();
    assert!(matches!(err, AicoreError::InvalidRequest(_)));
}

#[test]
fn infer_with_wrong_shape_fails() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();
    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 3], // Expectations [1],4
            data: vec![1.0, 0.5, 0.0],
        }],
    };
    let err = backend.infer(&handle, &req).unwrap_err();
    assert!(matches!(err, ContractError::AiInferenceFailed(_)));
}

#[test]
fn unload_removes_model() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();
    assert_eq!(backend.loaded_count(), 1);

    backend.unload(&handle).unwrap();
    assert_eq!(backend.loaded_count(), 0);

    // Twice failed
    assert!(backend.unload(&handle).is_err());
}

#[test]
fn infer_unknown_handle_fails() {
    use aicore_contracts::ai::runtime::ModelHandle;
    let backend = make_backend();
    let fake = ModelHandle {
        handle_id: "unknown".to_string(),
        model_name: "x".to_string(),
        model_version: "1".to_string(),
        loaded_at_unix_ms: 0,
    };
    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![1.0, 0.5, 0.0, 0.0],
        }],
    };
    let err = backend.infer(&fake, &req).unwrap_err();
    assert!(matches!(err, ContractError::ResourceUnavailable(_)));
}

#[test]
fn metrics_track_inferences() {
    let backend = make_backend();
    let handle = backend.load_model(&default_spec()).unwrap();
    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![1.0, 0.5, 0.0, 0.0],
        }],
    };

    // 2 successes
    backend.infer(&handle, &req).unwrap();
    backend.infer(&handle, &req).unwrap();

    // 1 failure
    let bad_req = InferenceRequest {
        features: vec![],
        tensors: vec![],
    };
    let _ = backend.infer(&handle, &bad_req);

    let snap = backend.metrics().snapshot();
    assert_eq!(snap.total_inferences, 3);
    assert_eq!(snap.successful, 2);
    assert_eq!(snap.failed, 1);
    assert_eq!(snap.timeouts, 0);
}

#[test]
fn multiple_models_coexist() {
    let backend = make_backend();
    let h1 = backend.load_model(&default_spec()).unwrap();
    let h2 = backend.load_model(&default_spec()).unwrap();
    assert_eq!(backend.loaded_count(), 2);

    backend.unload(&h1).unwrap();
    assert_eq!(backend.loaded_count(), 1);
    backend.unload(&h2).unwrap();
    assert_eq!(backend.loaded_count(), 0);
}

#[test]
fn pure_rust_and_onnx_coexist_as_separate_backends() {
    let onnx = make_backend();
    let pure = PureRustRuntime::new();

    assert_eq!(onnx.runtime_name(), "onnx_runtime_backend");
    assert_eq!(pure.runtime_name(), "pure_rust_runtime");

    assert!(onnx
        .capabilities()
        .contains(&RuntimeCapability::OnnxInference));
    assert!(pure
        .capabilities()
        .contains(&RuntimeCapability::FeatureInference));
}
