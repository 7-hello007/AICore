use crate::context::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// AI Provider 的输入。
///
/// 设计原则：
/// - 纯数据，不包含 trait object。
/// - base_policy 可选，因为 AI 可能被用于"仅理解上下文"的场景。
/// - memory 可选，因为不是所有 Provider 都消费历史。
#[derive(Debug, Clone, Default)]
pub struct AiInput {
    pub snapshot: ComputerSnapshot,
    pub context: ContextSnapshot,
    pub base_policy: Option<ResourcePolicy>,
    pub memory: Option<MemoryContext>,
}
