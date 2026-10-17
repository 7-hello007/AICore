use crate::errors::ContractResult;
use crate::policy::resource_policy::PowerMode;
use std::sync::{Mutex, MutexGuard};

static CONTROL_TRANSACTION: Mutex<()> = Mutex::new(());

/// Shared by policy and plugin control paths touching the same system resources.
/// Hold across capture, apply, readback and restoration; do not acquire recursively.
pub fn control_transaction_lock() -> ContractResult<MutexGuard<'static, ()>> {
    CONTROL_TRANSACTION.lock().map_err(|_| {
        crate::errors::ContractError::LockPoisoned("resource control transaction".into())
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PowerStateSnapshot {
    pub platform: String,
    /// Opaque adapter-owned identity, preserving custom OS profiles exactly.
    pub state_id: String,
}

/// System-level power policy is abstract.
pub trait IPowerController: Send + Sync {
    /// Coarse PowerMode projection alone does not preserve a custom native state.
    /// Native adapters opt in only when their snapshot preserves the exact state.
    fn supports_exact_state(&self) -> bool {
        false
    }
    /// Apply the power mode. Returns the mode in effect (the platform may be downgraded).
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode>;

    /// Reads the current power mode.
    fn get_profile(&self) -> ContractResult<PowerMode>;

    fn capture_state(&self) -> ContractResult<PowerStateSnapshot> {
        Ok(PowerStateSnapshot {
            platform: self.platform_name().into(),
            state_id: serde_json::to_string(&self.get_profile()?)
                .map_err(|e| crate::errors::ContractError::ContractInvalid(e.to_string()))?,
        })
    }
    fn restore_state(&self, state: &PowerStateSnapshot) -> ContractResult<()> {
        if state.platform != self.platform_name() {
            return Err(crate::errors::ContractError::ContractInvalid(
                "power state belongs to another adapter".into(),
            ));
        }
        let mode = serde_json::from_str(&state.state_id)
            .map_err(|e| crate::errors::ContractError::ContractInvalid(e.to_string()))?;
        if self.set_profile(mode)? != mode || self.get_profile()? != mode {
            return Err(crate::errors::ContractError::AdapterUnavailable(
                "power restoration readback differs".into(),
            ));
        }
        Ok(())
    }

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}
