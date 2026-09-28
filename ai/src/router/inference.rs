use std::sync::Arc;

use aicore_contracts::{
    ai::{input::AiInput, suggestion::AiSuggestion},
    errors::{ContractError, ContractResult},
    traits::ai_provider::IAiProvider,
};

/// Suggestion router.
///
/// Try IAiProvider in order of registration, return the first successful result.
/// Returns the last error when all fails.
///
/// Design purpose:
/// - Support the priority chain (local priority, backup, clouds) for multi-model / multi-Runtime.
/// - Provide a basis for future Model A → Model B → Model C replacement.
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

/// Pack InferenceRouter as IAiProvider for embedding in AiAssistedPolicyEngine.
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
