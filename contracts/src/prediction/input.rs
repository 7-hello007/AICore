use crate::application::id::ApplicationId;
use crate::context::snapshot::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::telemetry::snapshot::ComputerSnapshot;

/// 预测器输入。
///
/// 纯数据，不包含 trait object。
#[derive(Debug, Clone, Default)]
pub struct PredictionInput {
    pub application_id: Option<ApplicationId>,
    pub snapshot: ComputerSnapshot,
    pub context: ContextSnapshot,
    /// 可选的历史上下文；无 memory 时为 None。
    pub memory: Option<MemoryContext>,
    /// 预测窗口，毫秒。
    pub horizon_ms: u64,
}
