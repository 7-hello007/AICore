use crate::execution::result::ExecutionResult;
use crate::policy::resource_policy::ResourcePolicy;

/// 后置条件检查结果。
#[derive(Debug, Clone, PartialEq)]
pub enum PostConditionResult {
    /// 检查通过。
    Passed,
    /// 检查失败，附原因。
    Failed(String),
    /// 不适用（例如资源不可用），不算失败。
    NotApplicable,
}

/// 后置条件检查器。
///
/// 在 Apply 之后调用；若返回 `Failed`，Executor 应触发 Rollback。
pub trait IPostConditionChecker: Send + Sync {
    fn verify(&self, policy: &ResourcePolicy, result: &ExecutionResult) -> PostConditionResult;

    fn checker_name(&self) -> &'static str;
}
