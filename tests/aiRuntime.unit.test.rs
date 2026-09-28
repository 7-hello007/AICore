//! PureRustRuntime unit test.

use aicore_ai_runtime::{ModelWeights, PureRustRuntime};
use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, ModelKind, ModelSource, ModelSpec, RuntimeCapability, RuntimeHealth,
    },
    errors::ContractError,
    traits::ai_runtime::IAiRuntime,
};

fn inline_spec(name: &str, json: &str) -> ModelSpec {
    ModelSpec {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: json.to_string(),
        },
        onnx_spec: None,
    }
}

const GOOD_MODEL: &str = r#"{
    "schema_version": "1.0",
    "name": "test_model",
    "version": "1.0.0",
    "feature_dim": 4,
    "labels": ["boost_cpu", "boost_gpu", "keep_base"],
    "weights": [
        [0.9, 0.0, 0.0, 0.0],
        [0.0, 0.9, 0.0, 0.0],
        [0.1, 0.1, 0.1, 0.1]
    ]
}"#;

const BAD_DIM_MODEL: &str = r#"{
    "schema_version": "1.0",
    "name": "bad",
    "version": "1.0.0",
    "feature_dim": 4,
    "labels": ["a", "b"],
    "weights": [
        [0.9, 0.0, 0.0],
        [0.0, 0.9, 0.0, 0.0]
    ]
}"#;

const BAD_LABELS_MODEL: &str = r#"{
    "schema_version": "1.0",
    "name": "bad",
    "version": "1.0.0",
    "feature_dim": 4,
    "labels": ["a", "b"],
    "weights": [[0.0, 0.0, 0.0, 0.0]]
}"#;

#[test]
fn runtime_starts_not_ready() {
    let rt = PureRustRuntime::new();
    assert_eq!(rt.loaded_model_count(), 0);
    assert_eq!(rt.runtime_name(), "pure_rust_runtime");
    assert!(matches!(
        rt.health(),
        RuntimeHealth::Healthy { loaded_models: 0 }
    ));
}

#[test]
fn runtime_reports_capabilities() {
    let rt = PureRustRuntime::new();
    let caps = rt.capabilities();
    assert!(caps.contains(&RuntimeCapability::LoadFromFile));
    assert!(caps.contains(&RuntimeCapability::LoadFromMemory));
    assert!(caps.contains(&RuntimeCapability::FeatureInference));
}

#[test]
fn load_inline_model_succeeds() {
    let rt = PureRustRuntime::new();
    let handle = rt.load_model(&inline_spec("m", GOOD_MODEL)).unwrap();
    assert_eq!(handle.model_name, "test_model");
    assert_eq!(handle.model_version, "1.0.0");
    assert_eq!(rt.loaded_model_count(), 1);
}

#[test]
fn load_invalid_json_fails() {
    let rt = PureRustRuntime::new();
    let err = rt.load_model(&inline_spec("m", "{ not json")).unwrap_err();
    assert!(matches!(err, ContractError::InvalidRequest(_)));
    assert_eq!(rt.loaded_model_count(), 0);
}

#[test]
fn load_model_with_wrong_weight_dim_fails() {
    let rt = PureRustRuntime::new();
    let err = rt.load_model(&inline_spec("m", BAD_DIM_MODEL)).unwrap_err();
    assert!(matches!(err, ContractError::InvalidRequest(_)));
}

#[test]
fn load_model_with_mismatched_labels_fails() {
    let rt = PureRustRuntime::new();
    let err = rt
        .load_model(&inline_spec("m", BAD_LABELS_MODEL))
        .unwrap_err();
    assert!(matches!(err, ContractError::InvalidRequest(_)));
}

#[test]
fn load_file_source_succeeds() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../models/policy_classifier_v1.json"
    );
    let spec = ModelSpec {
        name: "policy_classifier_v1".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::File {
            path: path.to_string(),
        },
        onnx_spec: None,
    };
    let rt = PureRustRuntime::new();
    let handle = rt.load_model(&spec).unwrap();
    assert_eq!(handle.model_name, "policy_classifier_v1");
}

#[test]
fn load_file_missing_fails() {
    let spec = ModelSpec {
        name: "missing".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::File {
            path: "/nonexistent/model.json".to_string(),
        },
        onnx_spec: None,
    };
    let rt = PureRustRuntime::new();
    let err = rt.load_model(&spec).unwrap_err();
    assert!(matches!(err, ContractError::ResourceUnavailable(_)));
}

#[test]
fn infer_returns_softmax_scores() {
    let rt = PureRustRuntime::new();
    let handle = rt.load_model(&inline_spec("m", GOOD_MODEL)).unwrap();

    // High CPU → boost_cpu should win
    let resp = rt
        .infer(
            &handle,
            &InferenceRequest {
                features: vec![0.9, 0.0, 0.0, 0.0],
                tensors: Vec::new(),
            },
        )
        .unwrap();

    assert_eq!(resp.labels.len(), 3);
    assert_eq!(resp.labels[0].label, "boost_cpu");
    assert!(resp.labels[0].score > 0.5);
    assert_eq!(resp.model_version, "1.0.0");

    // Softmax Normalization
    let total: f32 = resp.labels.iter().map(|l| l.score).sum();
    assert!((total - 1.0).abs() < 0.001);
}

#[test]
fn infer_wrong_feature_dim_fails() {
    let rt = PureRustRuntime::new();
    let handle = rt.load_model(&inline_spec("m", GOOD_MODEL)).unwrap();
    let err = rt
        .infer(
            &handle,
            &InferenceRequest {
                features: vec![0.9],
                tensors: Vec::new(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, ContractError::InvalidRequest(_)));
}

#[test]
fn infer_with_unknown_handle_fails() {
    use aicore_contracts::ai::runtime::ModelHandle;
    let rt = PureRustRuntime::new();
    let fake = ModelHandle {
        handle_id: "unknown".to_string(),
        model_name: "x".to_string(),
        model_version: "1".to_string(),
        loaded_at_unix_ms: 0,
    };
    let err = rt
        .infer(
            &fake,
            &InferenceRequest {
                features: vec![0.5; 4],
                tensors: Vec::new(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, ContractError::ResourceUnavailable(_)));
}

#[test]
fn unload_model_succeeds() {
    let rt = PureRustRuntime::new();
    let handle = rt.load_model(&inline_spec("m", GOOD_MODEL)).unwrap();
    assert_eq!(rt.loaded_model_count(), 1);

    rt.unload(&handle).unwrap();
    assert_eq!(rt.loaded_model_count(), 0);

    // Secondary unmounting failed
    assert!(rt.unload(&handle).is_err());
}

#[test]
fn multiple_models_coexist() {
    let rt = PureRustRuntime::new();
    let h1 = rt.load_model(&inline_spec("m1", GOOD_MODEL)).unwrap();
    let h2 = rt.load_model(&inline_spec("m2", GOOD_MODEL)).unwrap();
    assert_eq!(rt.loaded_model_count(), 2);

    rt.unload(&h1).unwrap();
    assert_eq!(rt.loaded_model_count(), 1);

    rt.unload(&h2).unwrap();
    assert_eq!(rt.loaded_model_count(), 0);
}

#[test]
fn model_weights_infer_is_deterministic() {
    let w = serde_json::from_str::<ModelWeights>(GOOD_MODEL).unwrap();
    w.validate().unwrap();
    let a = w.infer(&[0.5, 0.5, 0.0, 0.0]).unwrap();
    let b = w.infer(&[0.5, 0.5, 0.0, 0.0]).unwrap();
    assert_eq!(a, b);
}
