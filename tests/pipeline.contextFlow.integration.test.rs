use std::sync::Arc;

use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    normalizer::TelemetryNormalizer,
};

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxTelemetryProvider as PlatformProvider;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsTelemetryProvider as PlatformProvider;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn telemetry_to_context_pipeline() {
    let provider = PlatformProvider::new();
    let snapshot = provider.collect().expect("collect");

    let normalizer = TelemetryNormalizer::new();
    let state = normalizer.normalize(snapshot);

    let engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let ctx = engine.build(&state);

    // 基本不变量
    assert!(ctx.timestamp_unix_ms.is_some());
    assert!(ctx.confidence >= 0.0 && ctx.confidence <= 1.0);
    assert!(ctx.active_apps.len() <= 16);
}
