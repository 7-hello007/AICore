use crate::execution::result::ExecutionResult;
use crate::policy::resource_policy::ResourcePolicy;

/// Post-condition check results.
#[derive(Debug, Clone, PartialEq)]
pub enum PostConditionResult {
    /// Check through.
    Passed,
    /// Check failed, with reasons.
    Failed(String),
    /// Not applicable (e.g., non-availability of resources) is not a failure.
    NotApplicable,
}

/// Post-condition checker.
///
/// Call after Apply; if `Failed` is returned, Excel should trigger Rollback.
pub trait IPostConditionChecker: Send + Sync {
    fn verify(&self, policy: &ResourcePolicy, result: &ExecutionResult) -> PostConditionResult;

    fn checker_name(&self) -> &'static str;
}
