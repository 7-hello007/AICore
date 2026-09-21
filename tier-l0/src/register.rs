use std::sync::Arc;

use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};
use aicore_core::context::WorkloadFusion;
use aicore_tier::TierHub;

use crate::adaptation::L0AdaptationProvider;
use crate::workload::L0WorkloadSource;

/// 默认 L0 TierId。
pub const L0_TIER_ID: &str = "l0.legacy";

/// L0 Tier 版本。
pub const L0_TIER_VERSION: &str = "1.0.0";

/// 只注册 L0 规则源。
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

/// 注册 L0 规则源 + 可选 ML 源，用 `WorkloadFusion` 组合。
///
/// `extra_sources` 可以包含 `MlWorkloadSource` 等。
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

    // Fusion 的 priority 设为 1000（顶层），但注册时以 100 声明（L0 优先级）。
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
