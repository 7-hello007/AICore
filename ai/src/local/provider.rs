use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    ai::{
        input::AiInput,
        model_metadata::{ModelMetadata, ModelRuntime, PrivacyClass},
        suggestion::{AiSource, AiSuggestion, AiSuggestionPayload},
        task::AiTaskType,
    },
    errors::{ContractError, ContractResult},
    policy::{
        priority::PolicyPriority,
        resource_policy::{PowerMode, ResourcePolicy},
    },
    traits::ai_provider::IAiProvider,
};

use super::features::FeatureVector;
use super::weights::{PolicyIntent, WeightTable};

/// 纯 Rust 特征加权分类器。
///
/// 行为：
/// 1. 从 `AiInput` 提取归一化特征。
/// 2. 对每个 intent 计算点积。
/// 3. softmax 得到概率分布。
/// 4. 若最高概率低于 `min_confidence`，返回 `AiInferenceFailed`，
///    让上层 `AiAssistedPolicyEngine` fallback 到规则引擎。
/// 5. 否则选最高分的 intent，转换为 policy 修改。
pub struct LocalFeatureProvider {
    weights: Arc<WeightTable>,
    available: bool,
    /// 最低置信度，低于此值不输出建议，直接失败。
    min_confidence: f32,
}

impl LocalFeatureProvider {
    pub fn new(weights: WeightTable) -> ContractResult<Self> {
        weights
            .validate()
            .map_err(|e| ContractError::InvalidRequest(format!("invalid weights: {}", e)))?;
        Ok(Self {
            weights: Arc::new(weights),
            available: true,
            min_confidence: 0.2,
        })
    }

    pub fn with_default_weights() -> ContractResult<Self> {
        Self::new(WeightTable::default_weights())
    }

    pub fn with_min_confidence(mut self, min: f32) -> Self {
        self.min_confidence = min.clamp(0.0, 1.0);
        self
    }

    pub fn min_confidence(&self) -> f32 {
        self.min_confidence
    }

    pub fn unavailable() -> Self {
        Self {
            weights: Arc::new(WeightTable::default_weights()),
            available: false,
            min_confidence: 0.2,
        }
    }

    pub fn weights(&self) -> &WeightTable {
        &self.weights
    }

    fn score_intents(&self, fv: &FeatureVector) -> Vec<(PolicyIntent, f32)> {
        PolicyIntent::ALL
            .iter()
            .filter_map(|&intent| {
                self.weights
                    .weights_for(intent)
                    .map(|w| (intent, fv.dot(&w)))
            })
            .collect()
    }
}

impl IAiProvider for LocalFeatureProvider {
    fn suggest(&self, input: &AiInput) -> ContractResult<AiSuggestion> {
        if !self.available {
            return Err(ContractError::AiInferenceFailed(
                "local_feature_provider is not available".to_string(),
            ));
        }

        let base_policy = input.base_policy.clone().unwrap_or_default();
        let fv = FeatureVector::from_input(input);

        let scores = self.score_intents(&fv);
        if scores.is_empty() {
            return Err(ContractError::AiInferenceFailed(
                "no valid weights available".to_string(),
            ));
        }

        // softmax
        let max_score = scores
            .iter()
            .map(|(_, s)| *s)
            .fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = scores.iter().map(|(_, s)| (s - max_score).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum).collect();

        let (best_idx, &best_prob) = probs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| ContractError::AiInferenceFailed("softmax failed".to_string()))?;

        // 低置信度：不输出强建议，让上层 fallback
        if best_prob < self.min_confidence {
            return Err(ContractError::AiInferenceFailed(format!(
                "confidence {:.3} below min {:.3}",
                best_prob, self.min_confidence
            )));
        }

        let (intent, _score) = scores[best_idx];
        let confidence = best_prob;

        let policy = apply_intent(&base_policy, intent);
        let reason = format!(
            "local_feature: intent={} confidence={:.3}",
            intent.name(),
            confidence
        );

        Ok(AiSuggestion {
            suggestion_id: generate_id(),
            source: AiSource::LocalModel,
            task: AiTaskType::RecommendPolicy,
            confidence,
            reason,
            created_at_unix_ms: now_unix_ms(),
            payload: AiSuggestionPayload::Policy(policy),
        })
    }

    fn name(&self) -> &'static str {
        "local_feature_provider"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["policy.recommend", "workload.classify"]
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: "local_feature_v1".to_string(),
            version: self.weights.version.clone(),
            runtime: ModelRuntime::NativeRust,
            capabilities: vec!["policy.recommend".to_string()],
            latency_budget_ms: 50,
            memory_budget_mb: 4,
            privacy_class: PrivacyClass::LocalOnly,
            description: "Pure-Rust feature-weighted policy classifier".to_string(),
        }
    }
}

fn apply_intent(base: &ResourcePolicy, intent: PolicyIntent) -> ResourcePolicy {
    let mut policy = base.clone();
    policy.source = aicore_contracts::policy::priority::PolicySource::AiRecommended;

    // 保证生成的 policy 满足基本不变量，
    // 即使 base 是 Default::default() 也能通过 Validator。
    if policy.policy_id.trim().is_empty() {
        policy.policy_id = generate_id();
    }
    if policy.created_at_unix_ms == 0 {
        policy.created_at_unix_ms = now_unix_ms();
    }

    match intent {
        PolicyIntent::BoostCpu => {
            policy.cpu.priority = PolicyPriority::High;
        },
        PolicyIntent::BoostGpu => {
            policy.gpu.priority = PolicyPriority::High;
        },
        PolicyIntent::BoostNpu => {
            policy.npu.priority = PolicyPriority::High;
            policy.npu.preferred = true;
        },
        PolicyIntent::ThrottleBackground => {
            policy.background.throttle = true;
            if policy.background.max_cpu_percent.is_none() {
                policy.background.max_cpu_percent = Some(20.0);
            }
        },
        PolicyIntent::PowerSave => {
            policy.power.mode = PowerMode::PowerSave;
        },
        PolicyIntent::KeepBase => {},
    }

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

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("local-{:x}", nanos)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
