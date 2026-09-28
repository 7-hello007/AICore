use crate::execution::{result::ExecutionResult, targets::ExecutionTargets};
use crate::policy::resource_policy::ResourcePolicy;

/// The policy implementer is abstract.
///
/// The input must be a ResourcePolicy that has passed IPolicyValidator.
pub trait IPolicyExecutor: Send + Sync {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult;

    fn name(&self) -> &'static str;
}
