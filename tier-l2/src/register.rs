use std::sync::Arc;

use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};
use aicore_contracts::traits::task_queue::ITaskQueue;
use aicore_tier::TierHub;

use crate::native_provider::L2NativeTaskProvider;

pub const L2_TIER_ID: &str = "l2.native";
pub const L2_TIER_VERSION: &str = "1.0.0";

/// Register L2 Tier and return (TierId, L2NativeTaskProvider handle).
pub fn register_l2(hub: &TierHub) -> Result<(TierId, Arc<L2NativeTaskProvider>), RegisterError> {
    register_l2_provider(hub, Arc::new(L2NativeTaskProvider::new()))
}

pub fn register_l2_with_queue(
    hub: &TierHub,
    queue: Arc<dyn ITaskQueue>,
) -> Result<(TierId, Arc<L2NativeTaskProvider>), RegisterError> {
    register_l2_provider(
        hub,
        Arc::new(L2NativeTaskProvider::new().with_task_queue(queue)),
    )
}

fn register_l2_provider(
    hub: &TierHub,
    provider: Arc<L2NativeTaskProvider>,
) -> Result<(TierId, Arc<L2NativeTaskProvider>), RegisterError> {
    let id = TierId::new(L2_TIER_ID);

    hub.register_native_task_provider(id.clone(), 1000, provider.clone())
        .map_err(|e| RegisterError::Capability(e.to_string()))?;

    let reg = TierRegistration::new(id.clone(), TierLevel::L2, L2_TIER_VERSION)
        .with_capability(TierCapability::NativeTaskControl)
        .with_capability(TierCapability::UserIntent)
        .with_description("L2 native integration: task lifecycle + reservation");

    match hub.register_tier(reg) {
        Ok(()) => Ok((id, provider)),
        Err(e) => {
            let _ = hub.unregister_tier(&id);
            Err(RegisterError::Registry(e.to_string()))
        },
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegisterError {
    #[error("capability registry error: {0}")]
    Capability(String),
    #[error("tier registry error: {0}")]
    Registry(String),
}
