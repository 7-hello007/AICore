use std::sync::Arc;

use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};
use aicore_core::context::WorkloadFusion;
use aicore_tier::TierHub;

use crate::adaptation::L0AdaptationProvider;
use crate::workload::L0WorkloadSource;

/// Default L0 TierId.
pub const L0_TIER_ID: &str = "l0.legacy";

/// L0 Tier version.
pub const L0_TIER_VERSION: &str = "1.0.0";

/// Only register L0 rule source.
pub fn register_l0(hub: &TierHub) -> Result<TierId, RegisterError> {
    let id = TierId::new(L0_TIER_ID);

    hub.register_workload_source(id.clone(), 100, Arc::new(L0WorkloadSource::new()))
        .map_err(|e| RegisterError::Capability(e.to_string()))?;
    hub.register_adaptation_provider(id.clone(), 100, Arc::new(L0AdaptationProvider::new()))
        .map_err(|e| RegisterError::Capability(e.to_string()))?;

    let reg = TierRegistration::new(id.clone(), TierLevel::L0, L0_TIER_VERSION)
        .with_capability(TierCapability::WorkloadObservation)
        .with_capability(TierCapability::TransparentAdaptation)
        .with_description("L0 legacy adapter");

    match hub.register_tier(reg) {
        Ok(()) => Ok(id),
        Err(e) => {
            let _ = hub.unregister_tier(&id);
            Err(RegisterError::Registry(e.to_string()))
        },
    }
}

/// Registers the L0 rule source + optional ML source in combination with `WorkloadFusion`.
///
/// `extra_sources` can contain `MlWorkloadSource` etc.
pub fn register_l0_with_fusion(
    hub: &TierHub,
    extra_sources: Vec<Arc<dyn aicore_contracts::traits::workload_source::IWorkloadSource>>,
) -> Result<TierId, RegisterError> {
    let id = TierId::new(L0_TIER_ID);

    let mut fusion = WorkloadFusion::new();
    fusion.add_source(Arc::new(L0WorkloadSource::new()));
    for s in extra_sources {
        fusion.add_source(s);
    }

    // The power of Fusion is set to 1000 (top level), but is declared at 100 (L0 priority).
    hub.register_workload_source(id.clone(), 100, Arc::new(fusion))
        .map_err(|e| RegisterError::Capability(e.to_string()))?;
    hub.register_adaptation_provider(id.clone(), 100, Arc::new(L0AdaptationProvider::new()))
        .map_err(|e| RegisterError::Capability(e.to_string()))?;

    let reg = TierRegistration::new(id.clone(), TierLevel::L0, L0_TIER_VERSION)
        .with_capability(TierCapability::WorkloadObservation)
        .with_capability(TierCapability::TransparentAdaptation)
        .with_description("L0 legacy adapter with fusion");

    match hub.register_tier(reg) {
        Ok(()) => Ok(id),
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
