use crate::errors::ContractResult;
use crate::execution::{
    result::{ExecutionOutcome, ExecutionResult},
    targets::ExecutionTargets,
};
use crate::policy::resource_policy::ResourcePolicy;

/// An adapter-owned original-state snapshot. Its lifetime may hold the executor's
/// transaction lock across apply, independent verification and restoration.
pub trait IPolicyTransaction {
    fn apply(&mut self) -> ExecutionResult;
    /// Restore every attempted resource and report each independent readback.
    fn restore(&mut self) -> Vec<ExecutionOutcome>;
}

/// The policy implementer is abstract.
///
/// The input must be a ResourcePolicy that has passed IPolicyValidator.
pub trait IPolicyExecutor: Send + Sync {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult;

    /// None preserves compatibility for stateless/model implementations. Such an
    /// implementation cannot claim original-state recovery after a failed apply.
    fn begin_transaction(
        &self,
        _policy: &ResourcePolicy,
        _targets: &ExecutionTargets,
    ) -> ContractResult<Option<Box<dyn IPolicyTransaction + '_>>> {
        Ok(None)
    }

    fn name(&self) -> &'static str;
}
