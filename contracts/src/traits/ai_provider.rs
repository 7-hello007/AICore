use crate::ai::{input::AiInput, model_metadata::ModelMetadata, suggestion::AiSuggestion};
use crate::errors::ContractResult;

/// AI Provider 抽象。
///
/// 实现可以是：
/// - 纯 Rust 本地推理（`LocalFeatureProvider`）
/// - ONNX Runtime（V3）
/// - llama.cpp（V3）
/// - 云端（需显式允许）
///
/// 所有实现都必须：
/// - 出错时返回 `ContractError`，不 panic。
/// - 不直接执行任何系统操作。
/// - 只输出 `AiSuggestion`。
/// - 声明 `ModelMetadata`。
pub trait IAiProvider: Send + Sync {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion>;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Vec<&'static str>;
    fn is_available(&self) -> bool;

    /// 返回模型元数据。默认返回 unknown。
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
