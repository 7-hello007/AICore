use crate::execution::{result::ExecutionResult, targets::ExecutionTargets};
use crate::policy::resource_policy::ResourcePolicy;

/// 策略执行器抽象。
///
/// 输入必须是已经通过 IPolicyValidator 的 ResourcePolicy。
pub trait IPolicyExecutor: Send + Sync {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult;

    fn name(&self) -> &'static str;
}
