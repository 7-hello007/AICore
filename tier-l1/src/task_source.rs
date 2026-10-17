use std::sync::RwLock;

use aicore_contracts::{
    application::id::ApplicationId,
    errors::{ContractError, ContractResult},
    task::contract::TaskContract,
    traits::task_source::ITaskSource,
};

/// L1 Task Source: Apply public API to declare the current task.
///
/// Semantics:
/// - Apply call `set_current(contract)` for "I'm doing X" now.
/// - `current_task()` returns the last stated condition.
/// - `clear()` indicates the active revocation of the declaration.
///
/// Features:
/// -** Do not initiate a rotation**: fully applied.
/// - ** Do not change TaskContract**: only store and return.
pub struct L1TaskSource {
    current: RwLock<Option<TaskContract>>,
}

impl L1TaskSource {
    pub fn try_set_current(&self, contract: TaskContract) -> ContractResult<()> {
        *self
            .current
            .try_write()
            .map_err(|_| ContractError::ResourceUnavailable("L1 source busy".into()))? =
            Some(contract);
        Ok(())
    }
    pub fn try_clear_for(&self, id: &ApplicationId) -> ContractResult<bool> {
        let mut slot = self
            .current
            .try_write()
            .map_err(|_| ContractError::ResourceUnavailable("L1 source busy".into()))?;
        if slot.as_ref().map(|t| &t.application_id) == Some(id) {
            *slot = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }

    /// Apply the declaration for the current task.
    pub fn set_current(&self, contract: TaskContract) -> ContractResult<()> {
        let mut slot = self.current.write().map_err(|_| {
            ContractError::AdapterUnavailable("l1 task source lock poisoned".into())
        })?;
        *slot = Some(contract);
        Ok(())
    }

    /// Apply to revoke the current declaration.
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

    /// Is there a declaration at this time?
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
        // L1 Manifest declaration priority above L0 extrapolation.
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
