use crate::control::trace::ControlTrace;
use crate::execution::{result::ExecutionResult, targets::ExecutionTargets};
use crate::policy::resource_policy::ResourcePolicy;

/// 透明控制执行器。
///
/// 与 V2 `IPolicyExecutor` 的区别：
/// - 六段式：Observe → Decide → Validate → Apply → Verify → Rollback。
/// - 每次执行产出 `ControlTrace`。
/// - 失败时回退到 `prior_policy`。
pub trait ITransparentExecutor: Send + Sync {
    /// 执行一条已验证的 policy，并在失败时回退到 `prior_policy`。
    ///
    /// `prior_policy` 为 `None` 时，失败后无回退目标，只记录回退失败。
    fn execute_transparent(
        &self,
        policy: &ResourcePolicy,
        targets: &ExecutionTargets,
        prior_policy: Option<&ResourcePolicy>,
    ) -> ControlTrace;

    /// 最近一次 Apply 的真实执行结果。实现可覆盖，用于审计/可观测性。
    fn last_execution_result(&self) -> Option<ExecutionResult> {
        None
    }

    fn name(&self) -> &'static str;
}
