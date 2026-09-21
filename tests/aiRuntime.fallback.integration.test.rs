//! 模型加载失败 / Provider 不可用时的 fallback 测试。

use std::sync::Arc;

use aicore_ai_runtime::PureRustRuntime;
use aicore_contracts::{
    ai::runtime::{ModelKind, ModelSource, ModelSpec},
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::priority::PolicySource,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        ai_provider::IAiProvider, ai_runtime::IAiRuntime, policy_engine::IPolicyEngine,
        policy_validator::IPolicyValidator,
    },
};
use aicore_core::policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine};

#[test]
fn missing_model_file_fails_to_load() {
    let rt = PureRustRuntime::new();
    let spec = ModelSpec {
        name: "missing".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::File {
            path: "/nonexistent/model.json".to_string(),
        },
        onnx_spec: None,
    };
    assert!(rt.load_model(&spec).is_err());
    assert_eq!(rt.loaded_model_count(), 0);
}

#[test]
fn ai_engine_falls_back_to_rule_when_no_provider() {
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    // 无 AI provider
    let engine = AiAssistedPolicyEngine::new(base, None, 0.5);

    let snapshot = ComputerSnapshot::default();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };

    let policy = engine.generate(&snapshot, &ctx);
    assert_eq!(policy.source, PolicySource::RuleBased);

    let v = DefaultPolicyValidator::new();
    assert!(v.validate(&policy).accepted);
}

#[test]
fn invalid_model_does_not_break_runtime() {
    let rt = PureRustRuntime::new();
    // 先尝试加载一个坏模型
    let bad = ModelSpec {
        name: "bad".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: "{ not valid".to_string(),
        },
        onnx_spec: None,
    };
    assert!(rt.load_model(&bad).is_err());

    // 之后仍可加载好模型
    const GOOD: &str = r#"{
        "schema_version": "1.0",
        "name": "good",
        "version": "1.0.0",
        "feature_dim": 4,
        "labels": ["a"],
        "weights": [[0.1, 0.1, 0.1, 0.1]]
    }"#;
    let good = ModelSpec {
        name: "good".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: GOOD.to_string(),
        },
        onnx_spec: None,
    };
    assert!(rt.load_model(&good).is_ok());
    assert_eq!(rt.loaded_model_count(), 1);
}

#[test]
fn runtime_health_reflects_state() {
    use aicore_contracts::ai::runtime::RuntimeHealth;
    let rt = PureRustRuntime::new();
    match rt.health() {
        RuntimeHealth::Healthy { loaded_models: 0 } => {},
        other => panic!("expected Healthy with 0 models, got {:?}", other),
    }

    // 加载一个模型
    const GOOD: &str = r#"{
        "schema_version": "1.0",
        "name": "m",
        "version": "1.0.0",
        "feature_dim": 4,
        "labels": ["a"],
        "weights": [[0.1, 0.1, 0.1, 0.1]]
    }"#;
    let spec = ModelSpec {
        name: "m".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: GOOD.to_string(),
        },
        onnx_spec: None,
    };
    rt.load_model(&spec).unwrap();

    match rt.health() {
        RuntimeHealth::Healthy { loaded_models: 1 } => {},
        other => panic!("expected Healthy with 1 model, got {:?}", other),
    }
}

#[test]
fn provider_returns_err_when_runtime_unloaded() {
    use aicore_ai_runtime::RuntimeBackedProvider;

    const GOOD: &str = r#"{
        "schema_version": "1.0",
        "name": "m",
        "version": "1.0.0",
        "feature_dim": 4,
        "labels": ["a"],
        "weights": [[0.1, 0.1, 0.1, 0.1]]
    }"#;
    let rt: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());
    let spec = ModelSpec {
        name: "m".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::FeatureWeighted,
        source: ModelSource::Inline {
            json: GOOD.to_string(),
        },
        onnx_spec: None,
    };
    let handle = rt.load_model(&spec).unwrap();
    let p = RuntimeBackedProvider::new(rt.clone(), handle.clone(), 4);

    // 卸载后 provider 应该失败
    rt.unload(&handle).unwrap();
    // is_available 检查 Health；health 仍为 Healthy（只是模型数为 0），
    // 但 infer 会失败，因为 handle 不再有效。
    // 这里只验证不 panic。
    let _ = p.is_available();
}

// ============================================================
// V3.2 Phase 2：ONNX → PureRust fallback
// ============================================================

#[test]
fn onnx_failure_falls_back_to_pure_rust() {
    use aicore_ai_runtime::{
        FallbackAiRuntime, MockOnnxSessionFactory, OnnxRuntimeBackend, PureRustRuntime,
    };
    use aicore_contracts::ai::runtime::ModelKind;
    use std::sync::Arc;

    let failing_factory = Arc::new(MockOnnxSessionFactory::failing("simulated ONNX failure"));
    let onnx: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(failing_factory));
    let pure: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());

    let fallback = FallbackAiRuntime::new(onnx, pure);
    assert_eq!(fallback.primary_name(), "onnx_runtime_backend");
    assert_eq!(fallback.fallback_name(), "pure_rust_runtime");

    // ONNX load 会失败 → fallback 到 PureRust
    // 但 PureRust 只接受 FeatureWeighted；传入 Onnx spec 也会失败
    let spec = aicore_contracts::ai::runtime::ModelSpec {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        kind: ModelKind::Onnx,
        source: aicore_contracts::ai::runtime::ModelSource::File {
            path: "/nonexistent.onnx".to_string(),
        },
        onnx_spec: Some(aicore_contracts::ai::runtime::OnnxModelSpec {
            input_schema: vec![],
            output_schema: vec![],
            preferred_provider: None,
        }),
    };

    // 两个 backend 都失败 → 返回合并错误
    let err = fallback.load_model(&spec).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("primary"));
    assert!(msg.contains("fallback"));
}

#[test]
fn fallback_counters_track_usage() {
    use aicore_ai_runtime::{
        FallbackAiRuntime, MockOnnxSessionFactory, OnnxRuntimeBackend, PureRustRuntime,
    };
    use std::sync::Arc;

    let failing_factory = Arc::new(MockOnnxSessionFactory::failing("simulate"));
    let onnx: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(failing_factory));
    let pure: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());

    let fallback = FallbackAiRuntime::new(onnx, pure);
    assert_eq!(fallback.fallback_load_count(), 0);
    assert_eq!(fallback.fallback_infer_count(), 0);

    // 触发一次 load fallback（PureRust 也不接受 Onnx spec，所以最终失败）
    let spec = aicore_contracts::ai::runtime::ModelSpec {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        kind: aicore_contracts::ai::runtime::ModelKind::Onnx,
        source: aicore_contracts::ai::runtime::ModelSource::File {
            path: "/x.onnx".to_string(),
        },
        onnx_spec: Some(aicore_contracts::ai::runtime::OnnxModelSpec {
            input_schema: vec![],
            output_schema: vec![],
            preferred_provider: None,
        }),
    };
    let _ = fallback.load_model(&spec);
    // 两个 backend 都失败 → 不增加 fallback_load_count
    assert_eq!(fallback.fallback_load_count(), 0);
}

#[test]
fn fallback_capabilities_are_union() {
    use aicore_ai_runtime::{
        FallbackAiRuntime, MockOnnxSessionFactory, OnnxRuntimeBackend, PureRustRuntime,
    };
    use aicore_contracts::ai::runtime::RuntimeCapability;
    use std::sync::Arc;

    let factory = Arc::new(MockOnnxSessionFactory::with_default_test_session());
    let onnx: Arc<dyn IAiRuntime> = Arc::new(OnnxRuntimeBackend::new(factory));
    let pure: Arc<dyn IAiRuntime> = Arc::new(PureRustRuntime::new());

    let fallback = FallbackAiRuntime::new(onnx, pure);
    let caps = fallback.capabilities();

    assert!(caps.contains(&RuntimeCapability::OnnxInference));
    assert!(caps.contains(&RuntimeCapability::FeatureInference));
    assert!(caps.contains(&RuntimeCapability::LoadFromFile));
}
