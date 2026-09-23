use std::sync::RwLock;

use aicore_contracts::{
    application::id::ApplicationId,
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

    /// Clear only a declaration owned by this application.
    pub fn clear_for(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        let mut slot = self.current.write().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        if slot.as_ref().map(|task| &task.application_id) == Some(application_id) {
            *slot = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 当前是否有声明。
    pub fn has_current(&self) -> bool {
        self.current_task().ok().flatten().is_some()
    }
}

impl Default for L1TaskSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ITaskSource for L1TaskSource {
    fn current_task(&self) -> ContractResult<Option<TaskContract>> {
        let mut slot = self.current.write().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        if slot
            .as_ref()
            .and_then(|task| task.deadline_unix_secs)
            .is_some_and(|deadline| deadline <= now_secs)
        {
            *slot = None;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_l1_declaration_is_not_reused() {
        let source = L1TaskSource::new();
        let mut contract = TaskContract::minimal(ApplicationId::new("app.test"));
        contract.deadline_unix_secs = Some(1);
        source.set_current(contract).unwrap();
        assert_eq!(source.current_task().unwrap(), None);
        assert!(!source.has_current());
    }
}
