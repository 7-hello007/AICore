use std::sync::Arc;

use aicore_contracts::{
    ai::{input::AiInput, suggestion::AiSuggestion},
    errors::{ContractError, ContractResult},
    traits::ai_provider::IAiProvider,
};

/// 推理路由器。
///
/// 按注册顺序尝试 IAiProvider，返回第一个成功结果。
/// 全部失败时返回最后一个错误。
///
/// 设计目的：
/// - 支持多模型 / 多 Runtime 的优先级链（本地优先、备用、云端）。
/// - 为将来的 Model A → Model B → Model C 替换提供基础。
pub struct InferenceRouter {
    providers: Vec<Arc<dyn IAiProvider>>,
}

impl InferenceRouter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn IAiProvider>) {
        self.providers.push(provider);
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        if self.providers.is_empty() {
            return Err(ContractError::AiInferenceFailed(
                "no AI provider registered".to_string(),
            ));
        }

        let mut last_error: Option<ContractError> = None;
        for provider in &self.providers {
            if !provider.is_available() {
                continue;
            }
            match provider.suggest(input) {
                Ok(suggestion) => return Ok(suggestion),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ContractError::AiInferenceFailed("all providers unavailable".to_string())
        }))
    }
}

impl Default for InferenceRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// 把 InferenceRouter 包装成 IAiProvider，以便嵌入 AiAssistedPolicyEngine。
pub struct RouterAsProvider {
    router: Arc<InferenceRouter>,
}

impl RouterAsProvider {
    pub fn new(router: Arc<InferenceRouter>) -> Self {
        Self { router }
    }
}

impl IAiProvider for RouterAsProvider {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        self.router.suggest(input)
    }

    fn name(&self) -> &'static str {
        "inference_router"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["policy.recommend", "multi_provider"]
    }

    fn is_available(&self) -> bool {
        self.router.provider_count() > 0
    }
}
