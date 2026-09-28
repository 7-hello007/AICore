use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    ai::{
        input::AiInput,
        suggestion::{AiSource, AiSuggestion, AiSuggestionPayload},
        task::AiTaskType,
    },
    context::workload::WorkloadType,
    errors::ContractResult,
    policy::priority::PolicyPriority,
    traits::ai_provider::IAiProvider,
};

/// Mark AI Provider.
///
/// Conduct:
/// - AI Reference workload and NPU is available: raise NPU priority, labeled.
/// - If memory suggests a historical preference for power-saving patterns and the current workload is not resource-intensive,
///   Reduce power mode to PowerSave.
/// - Other workload returns base_policy copy.
///
/// Confidence:
/// - Hit rule: 0.9
/// - Increase in hit history: extra + 0.05 (ceiling 1.0)
/// - No strong signal: 0.3
pub struct MockAiProvider {
    available: bool,
}

impl MockAiProvider {
    pub fn new() -> Self {
        Self { available: true }
    }

    pub fn unavailable() -> Self {
        Self { available: false }
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IAiProvider for MockAiProvider {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        let base_policy = input.base_policy.clone().unwrap_or_default();

        let memory_boost = input.memory.is_some();
        let prefers_power_save = input
            .memory
            .as_ref()
            .and_then(|m| m.preferred_power_mode)
            .map(|m| {
                matches!(
                    m,
                    aicore_contracts::policy::resource_policy::PowerMode::PowerSave
                )
            })
            .unwrap_or(false);

        let (payload, mut confidence, reason): (AiSuggestionPayload, f32, String) = match input
            .context
            .workload_type
        {
            WorkloadType::AiInference if input.snapshot.npu.available => {
                let mut policy = base_policy.clone();
                policy.npu.priority = PolicyPriority::High;
                policy.npu.preferred = true;
                if policy.gpu.priority == PolicyPriority::High {
                    policy.gpu.priority = PolicyPriority::Normal;
                }
                (
                    AiSuggestionPayload::Policy(policy),
                    0.9,
                    "AI inference detected, NPU available; prefer NPU over GPU".to_string(),
                )
            },
            WorkloadType::GeneralOffice | WorkloadType::FileProcessing if prefers_power_save => {
                let mut policy = base_policy.clone();
                policy.power.mode = aicore_contracts::policy::resource_policy::PowerMode::PowerSave;
                (
                    AiSuggestionPayload::Policy(policy),
                    0.75,
                    "history prefers power_save; apply to light workload".to_string(),
                )
            },
            WorkloadType::Development => (
                AiSuggestionPayload::Policy(base_policy.clone()),
                0.5,
                "development workload; keep base policy".to_string(),
            ),
            _ => (
                AiSuggestionPayload::Policy(base_policy.clone()),
                0.3,
                "no strong signal; fall back to base".to_string(),
            ),
        };

        if memory_boost && confidence >= 0.7 {
            confidence = (confidence + 0.05).min(1.0);
        }

        Ok(AiSuggestion {
            suggestion_id: generate_id(),
            source: AiSource::Mock,
            task: AiTaskType::RecommendPolicy,
            confidence,
            reason,
            created_at_unix_ms: now_unix_ms(),
            payload,
        })
    }

    fn name(&self) -> &'static str {
        "mock_ai_provider"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["policy.recommend"]
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("suggestion-{:x}", nanos)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
