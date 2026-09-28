use std::sync::Arc;

use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};
use aicore_tier::TierHub;

use crate::task_source::L1TaskSource;

pub const L1_TIER_ID: &str = "l1.aware";
pub const L1_TIER_VERSION: &str = "1.0.0";

/// Register L1 Tier and return (TierId, L1TaskSource handle).
///
/// Returns the `L1TaskSource` handle, and applies it to declare TaskContract.
pub fn register_l1(hub: &TierHub) -> Result<(TierId, Arc<L1TaskSource>), RegisterError> {
    let id = TierId::new(L1_TIER_ID);
    let source = Arc::new(L1TaskSource::new());

    hub.register_task_source(id.clone(), 500, source.clone())
        .map_err(|e| RegisterError::Capability(e.to_string()))?;

    let reg = TierRegistration::new(id.clone(), TierLevel::L1, L1_TIER_VERSION)
        .with_capability(TierCapability::TaskMetadata)
        .with_capability(TierCapability::ResourceHints)
        .with_description("L1 aware integration: app-declared task contracts");

    match hub.register_tier(reg) {
        Ok(()) => Ok((id, source)),
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
