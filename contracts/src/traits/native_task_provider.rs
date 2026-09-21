use crate::errors::ContractResult;
use crate::task::{contract::TaskContract, lifecycle::TaskState};

/// L2 原生任务生命周期句柄。
#[derive(Debug, Clone)]
pub struct NativeTaskHandle {
    pub handle_id: String,
}

/// 原生任务控制提供者。
///
/// **仅 L2 可实现**；L0 / L1 不得实现。
/// Core 通过此接口管理原生任务；不关心具体 Tier。
pub trait INativeTaskProvider: Send + Sync {
    fn submit(&self, contract: &TaskContract) -> ContractResult<NativeTaskHandle>;

    fn query(&self, handle: &NativeTaskHandle) -> ContractResult<TaskState>;

    fn cancel(&self, handle: &NativeTaskHandle) -> ContractResult<()>;

    fn source_name(&self) -> &'static str;
}
