use crate::application::id::ApplicationId;
use crate::errors::ContractResult;
use crate::task::contract::TaskContract;

/// Task 来源描述。
#[derive(Debug, Clone)]
pub struct TaskSourceInfo {
    pub application_id: ApplicationId,
    pub source: &'static str,
}

/// Task 元数据来源。
///
/// L0 从进程树推断；L1 通过 TaskContract 接收；L2 通过 native API 接收。
pub trait ITaskSource: Send + Sync {
    /// 拉取当前活跃的 Task（若可推断）。
    ///
    /// 返回 `Ok(None)` 表示当前没有可推断的 Task。
    fn current_task(&self) -> ContractResult<Option<TaskContract>>;

    fn source_name(&self) -> &'static str;

    fn priority(&self) -> u32 {
        100
    }
}
