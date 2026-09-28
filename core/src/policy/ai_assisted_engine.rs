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

/// AI Auxiliary Policy Engine.
///
/// Conduct:
/// 1. First call the base engine (ruled engine), get base_policy.
/// 2. If AI is not available, return directly to base_policy.
/// 3. Call `IAiProvider::suggest` if AI is available.
/// 4. If confidence is less than present, return base_policy.
/// 5. If Payload is recommended as Policy, the source of the tag is `AiRecommended` and returns.
/// The others returned base_policy.
///
/// Security engagement:
/// - Do not verify the validity of the AI recommendation - that is the function of `DefaultPolicyValidator`.
/// - AI cannot bypass Validator.
/// - AI failed automatically fallback to rule.
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

    /// Generates policy and returns (policy, ai_suggestion).
    /// ai_suggestion indicates that the rule path has been taken.
    pub fn generate_with_meta(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> (ResourcePolicy, Option<AiSuggestion>) {
        self.generate_full(snapshot, context, None)
    }

    /// with memory context generation policy.
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

/// Add an easy way to AiSuggestion.
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
