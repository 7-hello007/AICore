use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::runtime::route::RuntimeRoute;
use crate::telemetry::snapshot::ComputerSnapshot;

/// Runtime 路由决策。
pub trait IRuntimeRouter: Send + Sync {
    /// 根据当前状态与上下文决定路由。
    ///
    /// 约束：
    /// - 不得为"AI 化"而强行改变未知 workload。
    /// - 无明确收益 / 无法识别时返回 Native。
    /// - 出错时必须返回 Native，不 panic。
    fn decide(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<RuntimeRoute>;

    fn name(&self) -> &'static str;
}
