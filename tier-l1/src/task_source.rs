use std::sync::RwLock;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    task::contract::TaskContract,
    traits::task_source::ITaskSource,
};

/// L1 Task 来源：应用通过公开 API 声明当前任务。
///
/// 语义：
/// - 应用调用 `set_current(contract)` 声明"我现在在做 X"。
/// - `current_task()` 返回最近一次声明的 contract。
/// - `clear()` 表示应用主动撤销声明。
///
/// 特性：
/// - **不主动轮询**：完全由应用驱动。
/// - **不修改 TaskContract**：只存储与返回。
pub struct L1TaskSource {
    current: RwLock<Option<TaskContract>>,
}

impl L1TaskSource {
    pub fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }

    /// 应用声明当前任务。
    pub fn set_current(&self, contract: TaskContract) -> ContractResult<()> {
        let mut slot = self.current.write().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        *slot = Some(contract);
        Ok(())
    }

    /// 应用撤销当前声明。
    pub fn clear(&self) -> ContractResult<()> {
        let mut slot = self.current.write().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        *slot = None;
        Ok(())
    }

    /// 当前是否有声明。
    pub fn has_current(&self) -> bool {
        self.current.read().map(|s| s.is_some()).unwrap_or(false)
    }
}

impl Default for L1TaskSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ITaskSource for L1TaskSource {
    fn current_task(&self) -> ContractResult<Option<TaskContract>> {
        let slot = self.current.read().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        Ok(slot.clone())
    }

    fn source_name(&self) -> &'static str {
        "l1.task_source"
    }

    fn priority(&self) -> u32 {
        // L1 显式声明优先级高于 L0 推断。
        500
    }
}
