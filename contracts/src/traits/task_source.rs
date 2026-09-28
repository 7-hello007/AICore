use crate::application::id::ApplicationId;
use crate::errors::ContractResult;
use crate::task::contract::TaskContract;

/// Task Source Description.
#[derive(Debug, Clone)]
pub struct TaskSourceInfo {
    pub application_id: ApplicationId,
    pub source: &'static str,
}

/// Task metadata source.
///
/// L0 extrapolates from process tree; L1 receives via TaskContract; L2 receives through native API.
pub trait ITaskSource: Send + Sync {
    /// Draws currently active Task (if deduced).
    ///
    /// Returns `Ok(None)` to indicate that there is no extrapolation at this time.
    fn current_task(&self) -> ContractResult<Option<TaskContract>>;

    fn source_name(&self) -> &'static str;

    fn priority(&self) -> u32 {
        100
    }
}
