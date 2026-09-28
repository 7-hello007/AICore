//! OnnxRuntimeBackend timeout test.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use aicore_ai_runtime::{MockOnnxSessionFactory, OnnxRuntimeBackend};
use aicore_contracts::{
    ai::runtime::{
        InferenceLabel, InferenceRequest, ModelKind, ModelSource, ModelSpec, OnnxModelSpec,
        TensorDType, TensorInput, TensorSchema,
    },
    errors::ContractError,
    traits::ai_runtime::IAiRuntime,
};

use aicore_ai_runtime::onnx::session::{IOnnxSession, IOnnxSessionFactory};

struct SlowSession {
    delay: Duration,
}

impl IOnnxSession for SlowSession {
    fn run(&self, _inputs: &[TensorInput]) -> Result<Vec<InferenceLabel>, String> {
        std::thread::sleep(self.delay);
        Ok(vec![InferenceLabel {
            label: "slow".to_string(),
            score: 1.0,
        }])
    }
    fn input_schemas(&self) -> Vec<TensorSchema> {
        vec![TensorSchema {
            name: "input".to_string(),
            shape: vec![1, 4],
            dtype: TensorDType::F32,
        }]
    }
    fn output_schemas(&self) -> Vec<TensorSchema> {
        vec![]
    }
    fn provider_name(&self) -> &'static str {
        "slow_mock"
    }
}

struct SlowFactory {
    session: Arc<dyn IOnnxSession>,
}

impl IOnnxSessionFactory for SlowFactory {
    fn create_session(
        &self,
        _spec: &OnnxModelSpec,
        _path: &std::path::Path,
    ) -> Result<Arc<dyn IOnnxSession>, String> {
        Ok(self.session.clone())
    }
    fn factory_name(&self) -> &'static str {
        "slow_factory"
    }
}

fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("models/policy_classifier_v1.json")
}

fn spec() -> ModelSpec {
    ModelSpec {
        name: "slow_test".to_string(),
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

#[test]
fn slow_inference_beyond_timeout_returns_timeout_error() {
    let session: Arc<dyn IOnnxSession> = Arc::new(SlowSession {
        delay: Duration::from_millis(300),
    });
    let factory = Arc::new(SlowFactory { session });
    let backend = OnnxRuntimeBackend::new(factory).with_timeout(Duration::from_millis(50));

    let handle = backend.load_model(&spec()).unwrap();
    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![1.0, 0.5, 0.0, 0.0],
        }],
    };

    let err = backend.infer(&handle, &req).unwrap_err();
    assert!(
        matches!(err, ContractError::AiTimeout(_)),
        "expected AiTimeout, got: {:?}",
        err
    );

    // Record timeout indicators
    let snap = backend.metrics().snapshot();
    assert_eq!(snap.timeouts, 1);
    assert_eq!(snap.successful, 0);
}

#[test]
fn fast_inference_within_timeout_succeeds() {
    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    let backend = OnnxRuntimeBackend::new(factory).with_timeout(Duration::from_secs(5));

    let handle = backend.load_model(&spec()).unwrap();
    let req = InferenceRequest {
        features: vec![],
        tensors: vec![TensorInput {
            name: "input".to_string(),
            shape: vec![1, 4],
            data: vec![1.0, 0.5, 0.0, 0.0],
        }],
    };

    let resp = backend.infer(&handle, &req).unwrap();
    assert!(!resp.labels.is_empty());
}
