use std::sync::{mpsc, Arc};
use std::time::Duration;

use aicore_contracts::{
    ai::{input::AiInput, model_metadata::ModelMetadata, suggestion::AiSuggestion},
    errors::{ContractError, ContractResult},
    traits::ai_provider::IAiProvider,
};

type Reply = mpsc::SyncSender<ContractResult<AiSuggestion>>;

/// Runs inference on one bounded slow worker. The control caller waits no longer
/// than `timeout`, then falls back through the existing policy engine.
pub struct BoundedAiProvider {
    requests: mpsc::SyncSender<(AiInput, Reply)>,
    timeout: Duration,
    capabilities: Vec<&'static str>,
    metadata: ModelMetadata,
    available: bool,
}

impl BoundedAiProvider {
    pub fn new(provider: Arc<dyn IAiProvider>, timeout: Duration) -> ContractResult<Self> {
        let capabilities = provider.capabilities();
        let metadata = provider.metadata();
        let available = provider.is_available();
        let (sender, receiver) = mpsc::sync_channel::<(AiInput, Reply)>(1);
        std::thread::Builder::new()
            .name("aicore-slow-ai".to_string())
            .spawn(move || {
                while let Ok((input, reply)) = receiver.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        provider.suggest(&input)
                    }))
                    .unwrap_or_else(|_| {
                        Err(ContractError::AiInferenceFailed(
                            "AI provider panicked".to_string(),
                        ))
                    });
                    let _ = reply.send(result);
                }
            })
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;

        Ok(Self {
            requests: sender,
            timeout,
            capabilities,
            metadata,
            available,
        })
    }
}

impl IAiProvider for BoundedAiProvider {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        if !self.available {
            return Err(ContractError::AiInferenceFailed(
                "AI provider unavailable".to_string(),
            ));
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        self.requests
            .try_send((input.clone(), reply))
            .map_err(|error| {
                ContractError::AiTimeout(format!("AI worker busy or unavailable: {}", error))
            })?;
        receiver.recv_timeout(self.timeout).map_err(|error| {
            ContractError::AiTimeout(format!("AI inference deadline exceeded: {}", error))
        })?
    }

    fn name(&self) -> &'static str {
        "bounded_ai_provider"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        self.capabilities.clone()
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn metadata(&self) -> ModelMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::ai::suggestion::{AiSource, AiSuggestionPayload};

    struct SleepingProvider {
        delay: Duration,
    }

    impl IAiProvider for SleepingProvider {
        fn suggest(&self, _input: &AiInput) -> ContractResult<AiSuggestion> {
            std::thread::sleep(self.delay);
            Ok(AiSuggestion {
                suggestion_id: "mock".to_string(),
                source: AiSource::Mock,
                task: aicore_contracts::ai::task::AiTaskType::RecommendPolicy,
                confidence: 0.9,
                reason: "mock".to_string(),
                created_at_unix_ms: 0,
                payload: AiSuggestionPayload::Intent("test".to_string()),
            })
        }

        fn name(&self) -> &'static str {
            "sleeping_mock"
        }

        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }

        fn is_available(&self) -> bool {
            true
        }
    }

    #[test]
    fn slow_inference_times_out_without_blocking_control() {
        let provider = BoundedAiProvider::new(
            Arc::new(SleepingProvider {
                delay: Duration::from_millis(100),
            }),
            Duration::from_millis(5),
        )
        .unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(
            provider.suggest(&AiInput::default()),
            Err(ContractError::AiTimeout(_))
        ));
        assert!(started.elapsed() < Duration::from_millis(80));
    }

    #[test]
    fn timely_inference_returns_its_real_result() {
        let provider = BoundedAiProvider::new(
            Arc::new(SleepingProvider {
                delay: Duration::ZERO,
            }),
            Duration::from_millis(200),
        )
        .unwrap();
        assert_eq!(
            provider.suggest(&AiInput::default()).unwrap().suggestion_id,
            "mock"
        );
    }
}
