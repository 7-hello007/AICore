use crate::control::trace::ControlTrace;
use crate::execution::{result::ExecutionResult, targets::ExecutionTargets};
use crate::policy::resource_policy::ResourcePolicy;

/// Transparent control controller.
///
/// Different from V2 `IPolicyExecutor`:
/// - Six-part style: Observe Decide Validate Apply Verify Rollback.
/// - Each implementation output `ControlTrace`.
/// - On failure, restore captured original state when the executor supports it.
pub trait ITransparentExecutor: Send + Sync {
    /// Execute a validated policy and independently verify original-state recovery.
    ///
    /// `prior_policy` is advisory history for compatible implementations. It is
    /// not a resource snapshot and must not be reported as verified restoration.
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
