use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::runtime::route::RuntimeRoute;
use crate::telemetry::snapshot::ComputerSnapshot;

/// Runtime route decision.
pub trait IRuntimeRouter: Send + Sync {
    /// The route is determined according to the current status and context.
    ///
    /// Binding:
    /// - No forced change of unknown workload for AI.
    /// - No definite gain / Unidentifiable return to Native.
    /// - We must return Native, not Panic.
    fn decide(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<RuntimeRoute>;

    fn name(&self) -> &'static str;
}
