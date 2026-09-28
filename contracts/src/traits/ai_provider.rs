use crate::ai::{input::AiInput, model_metadata::ModelMetadata, suggestion::AiSuggestion};
use crate::errors::ContractResult;

/// AI Provider abstract.
///
/// This can be implemented by:
/// - Pure Rust local inference`LocalFeatureProvider`）
/// - ONNX Runtime（V3）
/// - llama.cpp（V3）
/// - Clouds.
///
/// All must be implemented by:
/// - Returns `ContractError` on error, not Panic.
/// - No direct system operation.
/// - Only output `AiSuggestion`.
/// - Statement `ModelMetadata`.
pub trait IAiProvider: Send + Sync {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion>;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Vec<&'static str>;
    fn is_available(&self) -> bool;

    /// Returns model metadata. Default returns unknown.
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: self.name().to_string(),
            version: "unknown".to_string(),
            runtime: crate::ai::model_metadata::ModelRuntime::Mock,
            capabilities: self.capabilities().iter().map(|s| s.to_string()).collect(),
            latency_budget_ms: 1000,
            memory_budget_mb: 64,
            privacy_class: crate::ai::model_metadata::PrivacyClass::LocalOnly,
            description: format!("{} (no metadata)", self.name()),
        }
    }
}
