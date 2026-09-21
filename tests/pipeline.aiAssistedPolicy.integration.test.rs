use std::sync::Arc;

use aicore_ai::MockAiProvider;
use aicore_contracts::traits::{
    policy_engine::IPolicyEngine, policy_validator::IPolicyValidator,
    telemetry_provider::ITelemetryProvider,
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    normalizer::TelemetryNormalizer,
    policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine},
};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn full_pipeline_with_ai_assisted_policy() {
    let provider = PlatformProvider::new();
    let snapshot = provider.collect().expect("collect");

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    let ctx_engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let ctx = ctx_engine.build(&state);

    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let ai_engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.6);

    let (policy, suggestion) = ai_engine.generate_with_meta(&state.snapshot, &ctx);

    // 无论走哪条路径，最终 policy 必须通过 Validator
    let validator = DefaultPolicyValidator::new();
    let result = validator.validate(&policy);
    assert!(
        result.accepted,
        "policy rejected: {:?} (suggestion: {:?})",
        result.issues,
        suggestion.map(|s| s.confidence)
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn full_pipeline_without_ai_still_works() {
    let provider = PlatformProvider::new();
    let snapshot = provider.collect().expect("collect");

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    let ctx_engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let ctx = ctx_engine.build(&state);

    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai_engine = AiAssistedPolicyEngine::new(base, None, 0.6);

    let policy = ai_engine.generate(&state.snapshot, &ctx);

    let validator = DefaultPolicyValidator::new();
    assert!(validator.validate(&policy).accepted);
}
