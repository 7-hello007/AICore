use std::sync::Arc;

use aicore_contracts::{
    ai::{
        input::AiInput,
        suggestion::{AiSuggestion, AiSuggestionPayload},
    },
    context::ContextSnapshot,
    memory::context::MemoryContext,
    policy::{priority::PolicySource, resource_policy::ResourcePolicy},
    telemetry::ComputerSnapshot,
    traits::{ai_provider::IAiProvider, policy_engine::IPolicyEngine},
};

/// AI 辅助策略引擎。
///
/// 行为：
/// 1. 先调用 base 引擎（规则引擎），得到 base_policy。
/// 2. 若 AI 不可用，直接返回 base_policy。
/// 3. 若 AI 可用，调用 `IAiProvider::suggest`。
/// 4. 若建议置信度低于 threshold，返回 base_policy。
/// 5. 若建议 payload 是 Policy，标记来源为 `AiRecommended` 后返回。
/// 6. 其他情况返回 base_policy。
///
/// 安全约定：
/// - 不校验 AI 建议的合法性——那是 `DefaultPolicyValidator` 的职责。
/// - AI 无法绕过 Validator。
/// - AI 失败自动 fallback 到规则。
pub struct AiAssistedPolicyEngine {
    base: Arc<dyn IPolicyEngine>,
    ai: Option<Arc<dyn IAiProvider>>,
    confidence_threshold: f32,
}

impl AiAssistedPolicyEngine {
    pub fn new(
        base: Arc<dyn IPolicyEngine>,
        ai: Option<Arc<dyn IAiProvider>>,
        confidence_threshold: f32,
    ) -> Self {
        Self {
            base,
            ai,
            confidence_threshold: confidence_threshold.clamp(0.0, 1.0),
        }
    }

    pub fn base_name(&self) -> &'static str {
        self.base.name()
    }

    pub fn ai_name(&self) -> Option<&'static str> {
        self.ai.as_ref().map(|a| a.name())
    }

    pub fn confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }

    /// 生成策略并返回 (policy, ai_suggestion)。
    /// ai_suggestion 为 None 表示走了规则路径。
    pub fn generate_with_meta(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> (ResourcePolicy, Option<AiSuggestion>) {
        self.generate_full(snapshot, context, None)
    }

    /// 带 memory 上下文生成策略。
    pub fn generate_full(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
        memory: Option<MemoryContext>,
    ) -> (ResourcePolicy, Option<AiSuggestion>) {
        let base_policy = self.base.generate(snapshot, context);

        let ai = match &self.ai {
            Some(a) if a.is_available() => a,
            _ => return (base_policy, None),
        };

        let input = AiInput {
            snapshot: snapshot.clone(),
            context: context.clone(),
            base_policy: Some(base_policy.clone()),
            memory,
        };

        let suggestion = match ai.suggest(&input) {
            Ok(s) => s,
            Err(_) => return (base_policy, None),
        };

        if suggestion.confidence < self.confidence_threshold {
            return (base_policy, Some(suggestion));
        }

        match &suggestion.payload {
            AiSuggestionPayload::Policy(ai_policy) => {
                let merged = ResourcePolicy {
                    source: PolicySource::AiRecommended,
                    reason: format!(
                        "ai[{}]: {} | base: {}",
                        suggestion.source_str(),
                        suggestion.reason,
                        base_policy.reason
                    ),
                    ..ai_policy.clone()
                };
                (merged, Some(suggestion))
            },
            _ => (base_policy, Some(suggestion)),
        }
    }
}

impl IPolicyEngine for AiAssistedPolicyEngine {
    fn generate(&self, snapshot: &ComputerSnapshot, context: &ContextSnapshot) -> ResourcePolicy {
        self.generate_full(snapshot, context, None).0
    }

    fn generate_with_memory(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
        memory: Option<MemoryContext>,
    ) -> ResourcePolicy {
        self.generate_full(snapshot, context, memory).0
    }

    fn name(&self) -> &'static str {
        "ai_assisted_policy_engine"
    }
}

/// 给 AiSuggestion 加一个便捷方法。
trait SuggestionSourceStr {
    fn source_str(&self) -> &'static str;
}

impl SuggestionSourceStr for AiSuggestion {
    fn source_str(&self) -> &'static str {
        use aicore_contracts::ai::suggestion::AiSource;
        match self.source {
            AiSource::Mock => "mock",
            AiSource::LocalModel => "local_model",
            AiSource::SmallLanguageModel => "slm",
            AiSource::CloudModel => "cloud",
        }
    }
}
