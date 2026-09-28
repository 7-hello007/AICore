use crate::context::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// Policy Engineering interface.
pub trait IPolicyEngine: Send + Sync {
    fn generate(&self, snapshot: &ComputerSnapshot, context: &ContextSnapshot) -> ResourcePolicy;

    /// with memory context generation policy.
    ///
    /// The default implementation ignores memory and falls back to `generate`.
    /// The Memory-aware engine (e.g. `AiAssistedPolicyEngine`) should rewrite this method.
    fn generate_with_memory(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
        _memory: Option<MemoryContext>,
    ) -> ResourcePolicy {
        self.generate(snapshot, context)
    }

    fn name(&self) -> &'static str;
}
