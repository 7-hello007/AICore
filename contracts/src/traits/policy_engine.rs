use crate::context::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// Policy Engine 接口。
pub trait IPolicyEngine: Send + Sync {
    fn generate(&self, snapshot: &ComputerSnapshot, context: &ContextSnapshot) -> ResourcePolicy;

    /// 带 memory 上下文生成策略。
    ///
    /// 默认实现忽略 memory，退化为 `generate`。
    /// Memory-aware 引擎（例如 `AiAssistedPolicyEngine`）应重写此方法。
    fn generate_with_memory(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
        _memory: Option<MemoryContext>,
    ) -> ResourcePolicy {
        self.generate(snapshot, context)
    }

    fn name(&self) -> &'static str;
}
