//! DefaultRuntimeBinder unit test.

use std::path::PathBuf;
use std::sync::Arc;

use aicore_ai_runtime::{MockOnnxSessionFactory, OnnxRuntimeBackend, PureRustRuntime};
use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, ModelKind, ModelSource, ModelSpec, OnnxModelSpec, RuntimeCapability,
        TensorDType, TensorInput, TensorSchema,
    },
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    hardware::capability::{HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet},
    runtime::route::{RuntimeRoute, RuntimeRouteSource},
    traits::{
        ai_runtime::IAiRuntime,
        runtime_binder::{BindingRequest, IRuntimeBinder},
    },
};
use aicore_runtime_router::DefaultRuntimeBinder;

fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("models/policy_classifier_v1.json")
}

fn route(source: RuntimeRouteSource) -> RuntimeRoute {
    RuntimeRoute {
        route_id: "route-1".to_string(),
        workload_type: WorkloadType::AiInference,
        source,
        applied: source != RuntimeRouteSource::Native,
        reason: "test".to_string(),
        confidence: 0.9,
        decided_at_unix_secs: 1_700_000_000,
    }
}

fn ctx() -> ContextSnapshot {
    ContextSnapshot {
        workload_type: WorkloadType::AiInference,
        interaction_level: InteractionLevel::Interactive,
        confidence: 0.9,
        ..Default::default()
    }
}

fn onnx_backend() -> (
    Arc<dyn IAiRuntime>,
    aicore_contracts::ai::runtime::ModelHandle,
) {
    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    let rt: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(factory));
    let spec = ModelSpec {
        name: "t".to_string(),
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
    };
    let handle = rt.load_model(&spec).unwrap();
    (rt, handle)
}

#[test]
fn binder_name() {
    let b = DefaultRuntimeBinder::new();
    assert_eq!(b.name(), "default_runtime_binder");
    assert!(!b.has_runtime());
    assert!(b.loaded_models().is_empty());
}

#[test]
fn native_route_not_executed() {
    let b = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: route(RuntimeRouteSource::Native),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.reason.contains("native"));
}

#[test]
fn ai_runtime_route_without_backend_not_executed() {
    let b = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: route(RuntimeRouteSource::AiRuntime),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.reason.contains("no IAiRuntime"));
}

#[test]
fn ai_runtime_route_without_inference_does_not_claim_execution() {
    let (rt, handle) = onnx_backend();
    let b = DefaultRuntimeBinder::new().with_ai_runtime(rt, handle);
    let req = BindingRequest {
        route: route(RuntimeRouteSource::AiRuntime),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.inference.is_none());
    assert!(res.reason.contains("no inference payload"));
}

#[test]
fn ai_runtime_route_with_inference_executes() {
    let (rt, handle) = onnx_backend();
    let b = DefaultRuntimeBinder::new().with_ai_runtime(rt, handle);
    let req = BindingRequest {
        route: route(RuntimeRouteSource::AiRuntime),
        context: ctx(),
        inference: Some(InferenceRequest {
            features: vec![],
            tensors: vec![TensorInput {
                name: "input".to_string(),
                shape: vec![1, 4],
                data: vec![0.9, 0.1, 0.0, 0.0],
            }],
        }),
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(res.executed);
    assert!(res.inference.is_some());
    assert_eq!(res.inference.as_ref().unwrap().labels.len(), 4);
}

#[test]
fn cpu_route_without_inference_does_not_claim_execution() {
    let (rt, handle) = onnx_backend();
    let b = DefaultRuntimeBinder::new().with_ai_runtime(rt, handle);
    let req = BindingRequest {
        route: route(RuntimeRouteSource::Cpu),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.inference.is_none());
}

#[test]
fn gpu_route_without_capability_not_executed() {
    let b = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: route(RuntimeRouteSource::Gpu),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.reason.contains("no GPU capability"));
}

#[test]
fn gpu_route_with_capability_but_no_executor_degraded() {
    // Phase 4: GPU capacity but not registered executor downgrade (not executed).
    let caps = HardwareCapabilitySet::new("gpu:test").with(HardwareCapabilityEntry::supported(
        HardwareCapability::DeviceIdentity,
    ));
    let b = DefaultRuntimeBinder::new().with_hardware(caps);
    let req = BindingRequest {
        route: route(RuntimeRouteSource::Gpu),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(
        res.reason.contains("no executor registered"),
        "unexpected reason: {}",
        res.reason
    );
}

#[test]
fn npu_route_returns_degraded() {
    let b = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: route(RuntimeRouteSource::Npu),
        context: ctx(),
        inference: None,
    };
    let res = b.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.reason.contains("no NPU capability"));
}

#[test]
fn pure_rust_runtime_can_be_bound() {
    let rt: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());
    let json = r#"{
        "schema_version": "1.0",
        "name": "t",
        "version": "1.0.0",
        "feature_dim": 4,
        "labels": ["a"],
        "weights": [[0.1, 0.1, 0.1, 0.1]]
    }"#;
    let spec = ModelSpec {
        name: "t".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: json.to_string(),
        },
        onnx_spec: None,
    };
    let handle = rt.load_model(&spec).unwrap();

    let b = DefaultRuntimeBinder::new().with_ai_runtime(rt.clone(), handle.clone());
    assert!(b.has_runtime());
    assert_eq!(b.loaded_models().len(), 1);

    let caps = rt.capabilities();
    assert!(caps.contains(&RuntimeCapability::FeatureInference));
}
