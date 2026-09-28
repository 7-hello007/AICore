use crate::control::trace::ControlTrace;
use crate::execution::{result::ExecutionResult, targets::ExecutionTargets};
use crate::policy::resource_policy::ResourcePolicy;

/// Transparent control controller.
///
/// Different from V2 `IPolicyExecutor`:
/// - Six-part style: Observe Decide Validate Apply Verify Rollback.
/// - Each implementation output `ControlTrace`.
/// - Back to `prior_policy` if it fails.
pub trait ITransparentExecutor: Send + Sync {
    /// Execute a validated policy and back to `prior_policy` if it fails.
    ///
    /// When `prior_policy` is `None`, the failure does not result in a return target, but only a return failure is recorded.
    fn execute_transparent(
        &self,
        policy: &ResourcePolicy,
        targets: &ExecutionTargets,
        prior_policy: Option<&ResourcePolicy>,
    ) -> ControlTrace;

    /// The latest Apply real implementation results.
    fn last_execution_result(&self) -> Option<ExecutionResult> {
        None
    }

    fn name(&self) -> &'static str;
}
