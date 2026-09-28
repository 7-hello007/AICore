use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    ai::{
        input::AiInput,
        runtime::{InferenceRequest, ModelHandle, RuntimeHealth},
        suggestion::{AiSource, AiSuggestion, AiSuggestionPayload},
        task::AiTaskType,
    },
    errors::{ContractError, ContractResult},
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{PowerMode, ResourcePolicy},
    },
    traits::{ai_provider::IAiProvider, ai_runtime::IAiRuntime},
};

/// `IAiProvider` driven by `IAiRuntime`.
///
/// Process: `AiInput`→ feature extract →`runtime.infer` → tag map →policy proposal.
pub struct RuntimeBackedProvider {
    runtime: Arc<dyn IAiRuntime>,
    model_handle: ModelHandle,
    feature_dim: usize,
    min_confidence: f32,
}

impl RuntimeBackedProvider {
    pub fn new(
        runtime: Arc<dyn IAiRuntime>,
        model_handle: ModelHandle,
        feature_dim: usize,
    ) -> Self {
        Self {
            runtime,
            model_handle,
            feature_dim,
            min_confidence: 0.6,
        }
    }

    pub fn with_min_confidence(mut self, c: f32) -> Self {
        self.min_confidence = c.clamp(0.0, 1.0);
        self
    }

    pub fn min_confidence(&self) -> f32 {
        self.min_confidence
    }

    pub fn model_handle(&self) -> &ModelHandle {
        &self.model_handle
    }

    /// Expose the same runtime instance for system-level routing without exposing
    /// the concrete model implementation to Core.
    pub fn runtime(&self) -> Arc<dyn IAiRuntime> {
        self.runtime.clone()
    }

    /// Extracts the normalized feature from `AiInput`.
    ///
    /// Former 4D fixed to cpu / gpu / npu / mem;
    /// If `feature_dim` > 4, fill 0 for the rest.
    fn features(&self, input: &AiInput) -> Vec<f32> {
        let mut f = vec![
            (input.snapshot.cpu.usage_percent / 100.0).clamp(0.0, 1.0),
            (input.snapshot.gpu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
            (input.snapshot.npu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
            (input.snapshot.memory.usage_percent / 100.0).clamp(0.0, 1.0),
        ];
        match f.len().cmp(&self.feature_dim) {
            std::cmp::Ordering::Less => f.resize(self.feature_dim, 0.0),
            std::cmp::Ordering::Greater => f.truncate(self.feature_dim),
            std::cmp::Ordering::Equal => {},
        }
        f
    }

    /// Moves the tab to policy.
    fn apply_label(&self, base: &ResourcePolicy, label: &str) -> ResourcePolicy {
        let mut policy = base.clone();
        policy.source = PolicySource::AiRecommended;

        if policy.policy_id.trim().is_empty() {
            policy.policy_id = generate_id();
        }
        if policy.created_at_unix_ms == 0 {
            policy.created_at_unix_ms = now_unix_ms();
        }

        match label {
            "boost_cpu" => policy.cpu.priority = PolicyPriority::High,
            "boost_gpu" => policy.gpu.priority = PolicyPriority::High,
            "boost_npu" => {
                policy.npu.priority = PolicyPriority::High;
                policy.npu.preferred = true;
            },
            "power_save" => policy.power.mode = PowerMode::PowerSave,
            "throttle_background" => {
                policy.background.throttle = true;
                if policy.background.max_cpu_percent.is_none() {
                    policy.background.max_cpu_percent = Some(20.0);
                }
            },
            "keep_base" => {},
            _ => {},
        }

        // Maintenance: overall > = all sub-resources
        let overall = [
            policy.cpu.priority,
            policy.gpu.priority,
            policy.npu.priority,
            policy.memory.priority,
        ]
        .into_iter()
        .max()
        .unwrap_or(policy.priority);
        policy.priority = overall;
        policy
    }
}

impl IAiProvider for RuntimeBackedProvider {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        let features = self.features(input);
        let request = InferenceRequest {
            features,
            tensors: Vec::new(),
        };
        let response = self.runtime.infer(&self.model_handle, &request)?;

        let top = response
            .labels
            .first()
            .ok_or_else(|| ContractError::AiInferenceFailed("no inference output".to_string()))?;

        if top.score < self.min_confidence {
            return Err(ContractError::AiInferenceFailed(format!(
                "confidence {:.3} below min {:.3}",
                top.score, self.min_confidence
            )));
        }

        let base = input.base_policy.clone().unwrap_or_default();
        let policy = self.apply_label(&base, &top.label);

        Ok(AiSuggestion {
            suggestion_id: generate_id(),
            source: AiSource::LocalModel,
            task: AiTaskType::RecommendPolicy,
            confidence: top.score,
            reason: format!(
                "runtime[{}]: label={} score={:.3} latency={}ms",
                self.runtime.runtime_name(),
                top.label,
                top.score,
                response.latency_ms
            ),
            created_at_unix_ms: now_unix_ms(),
            payload: AiSuggestionPayload::Policy(policy),
        })
    }

    fn name(&self) -> &'static str {
        "runtime_backed_provider"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["policy.recommend"]
    }

    fn is_available(&self) -> bool {
        matches!(self.runtime.health(), RuntimeHealth::Healthy { .. })
    }
}

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("runtime-{:x}", nanos)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
