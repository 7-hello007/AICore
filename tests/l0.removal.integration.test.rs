//! L0 test: Delete L0 without prejudice to other Tiers and Core.

use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::ContractResult,
    telemetry::snapshot::ComputerSnapshot,
    tier::{
        capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
    },
    traits::workload_source::{IWorkloadSource, WorkloadObservation},
};
use aicore_tier::TierHub;
use aicore_tier_l0::register_l0;

/// Mark L1 workload source; simulation of "declaration" source.
struct MockL1Source;
impl IWorkloadSource for MockL1Source {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        Ok(WorkloadObservation {
            workload_type: WorkloadType::AiInference,
            confidence: 0.95,
            evidence: vec!["l1 declaration".to_string()],
        })
    }
    fn source_name(&self) -> &'static str {
        "l1"
    }
    fn priority(&self) -> u32 {
        200
    }
}

#[test]
fn remove_l0_keeps_l1_operational() {
    let hub = TierHub::new();

    // Register L0 (real)
    let l0_id = register_l0(&hub).unwrap();

    // Register mark L1
    let l1_id = TierId::new("l1.aware");
    hub.register_tier(
        TierRegistration::new(l1_id.clone(), TierLevel::L1, "1.0.0")
            .with_capability(TierCapability::WorkloadObservation),
    )
    .unwrap();
    hub.register_workload_source(l1_id.clone(), 200, Arc::new(MockL1Source))
        .unwrap();

    // L1 higher priority
    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "l1");

    // Delete L0
    hub.unregister_tier(&l0_id).unwrap();

    // L1 still available
    let best_after = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best_after.source_name(), "l1");

    // The metadata is only L1.
    assert_eq!(hub.registry().count().unwrap(), 1);
    let remaining = hub.registry().list().unwrap();
    assert_eq!(remaining[0].id.as_str(), "l1.aware");
}

#[test]
fn remove_l0_leaves_empty_hub_valid() {
    let hub = TierHub::new();
    let l0_id = register_l0(&hub).unwrap();

    assert_eq!(hub.registry().count().unwrap(), 1);
    assert!(hub.best_workload_source().unwrap().is_some());

    hub.unregister_tier(&l0_id).unwrap();

    // Empty Hub still valid
    assert!(hub.registry().is_empty().unwrap());
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.best_adaptation_provider().unwrap().is_none());
}

#[test]
fn re_register_l0_after_removal() {
    let hub = TierHub::new();
    let l0_id = register_l0(&hub).unwrap();
    hub.unregister_tier(&l0_id).unwrap();

    // Re-registration should be successful
    let l0_id2 = register_l0(&hub).unwrap();
    assert_eq!(l0_id2.as_str(), l0_id.as_str());
    assert!(hub.best_workload_source().unwrap().is_some());
}
