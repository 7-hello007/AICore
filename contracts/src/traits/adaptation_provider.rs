use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// Transparent fit proposal.
#[derive(Debug, Clone)]
pub struct AdaptationHint {
    /// Suggests to overwrite policy (None means not overwrite).
    pub policy_override: Option<ResourcePolicy>,
    /// Human readable reasons.
    pub reason: String,
    /// Confidence, 0.0-1.0.
    pub confidence: f32,
}

/// Transparent adaptation providers.
///
/// L0 by observation; L1 by declaration; L2 by native API.
/// Core obtains recommendations through this interface; it does not concern the specific Tier.
pub trait IAdaptationProvider: Send + Sync {
    fn suggest(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<Option<AdaptationHint>>;

    fn source_name(&self) -> &'static str;

    fn priority(&self) -> u32 {
        100
    }
}
