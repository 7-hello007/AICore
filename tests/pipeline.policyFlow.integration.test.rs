use std::sync::Arc;

use aicore_contracts::traits::{
    policy_engine::IPolicyEngine, policy_validator::IPolicyValidator,
    telemetry_provider::ITelemetryProvider,
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    normalizer::TelemetryNormalizer,
    policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn telemetry_to_context_to_policy_to_validator() {
    let provider = PlatformProvider::new();
    let snapshot = provider.collect().expect("collect");

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    let ctx_engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let ctx = ctx_engine.build(&state);

    let policy_engine = RuleBasedPolicyEngine::new();
    let policy = policy_engine.generate(&state.snapshot, &ctx);

    let validator = DefaultPolicyValidator::new();
    let result = validator.validate(&policy);

    assert!(result.accepted, "policy rejected: {:?}", result.issues);
    assert!(!policy.policy_id.is_empty());
}
