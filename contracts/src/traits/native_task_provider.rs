use crate::errors::ContractResult;
use crate::task::{contract::TaskContract, lifecycle::TaskState};

/// L2 Natural mission life cycle handle.
#[derive(Debug, Clone)]
pub struct NativeTaskHandle {
    pub handle_id: String,
}

/// Primary task control provider.
///
/// **Implemented only by L2**; L0 and L1 do not implement this contract.
/// Core manages primary tasks through this interface; it does not concern the specific Tier.
pub trait INativeTaskProvider: Send + Sync {
    fn submit(&self, contract: &TaskContract) -> ContractResult<NativeTaskHandle>;

    fn query(&self, handle: &NativeTaskHandle) -> ContractResult<TaskState>;

    fn cancel(&self, handle: &NativeTaskHandle) -> ContractResult<()>;

    fn source_name(&self) -> &'static str;
}
