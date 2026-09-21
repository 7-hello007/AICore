//! TierHub + CapabilityDiscovery 测试。

use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    telemetry::snapshot::ComputerSnapshot,
    tier::{
        capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
    },
    traits::workload_source::{IWorkloadSource, WorkloadObservation},
};
use aicore_tier::TierHub;

struct MockWorkloadSource {
    name: &'static str,
    workload: WorkloadType,
}

impl IWorkloadSource for MockWorkloadSource {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        Ok(WorkloadObservation {
            workload_type: self.workload,
            confidence: 0.9,
            evidence: vec![format!("from {}", self.name)],
        })
    }
    fn source_name(&self) -> &'static str {
        self.name
    }
}

fn reg(id: &str, level: TierLevel, caps: Vec<TierCapability>) -> TierRegistration {
    TierRegistration::new(TierId::new(id), level, "1.0.0").with_capabilities(caps)
}

#[test]
fn empty_hub_is_valid() {
    let hub = TierHub::new();
    assert!(hub.registry().is_empty().unwrap());
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.best_task_source().unwrap().is_none());
    assert!(hub.best_adaptation_provider().unwrap().is_none());
    assert!(hub.best_native_task_provider().unwrap().is_none());
}

#[test]
fn register_tier_and_capability() {
    let hub = TierHub::new();
    let id = TierId::new("l0.legacy");
    hub.register_tier(reg(
        "l0.legacy",
        TierLevel::L0,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();

    let provider: Arc<dyn IWorkloadSource> = Arc::new(MockWorkloadSource {
        name: "l0",
        workload: WorkloadType::Development,
    });
    hub.register_workload_source(id.clone(), 100, provider)
        .unwrap();

    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "l0");
}

#[test]
fn priority_determines_best() {
    let hub = TierHub::new();
    let id_lo = TierId::new("lo");
    let id_hi = TierId::new("hi");

    hub.register_workload_source(
        id_lo,
        50,
        Arc::new(MockWorkloadSource {
            name: "lo",
            workload: WorkloadType::GeneralOffice,
        }),
    )
    .unwrap();
    hub.register_workload_source(
        id_hi,
        200,
        Arc::new(MockWorkloadSource {
            name: "hi",
            workload: WorkloadType::AiInference,
        }),
    )
    .unwrap();

    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "hi");

    let all = hub.all_workload_sources().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].source_name(), "hi");
    assert_eq!(all[1].source_name(), "lo");
}

#[test]
fn unregister_tier_removes_capability() {
    let hub = TierHub::new();
    let id = TierId::new("l0.legacy");
    hub.register_tier(reg(
        "l0.legacy",
        TierLevel::L0,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();
    hub.register_workload_source(
        id.clone(),
        100,
        Arc::new(MockWorkloadSource {
            name: "l0",
            workload: WorkloadType::Development,
        }),
    )
    .unwrap();

    assert!(hub.best_workload_source().unwrap().is_some());

    hub.unregister_tier(&id).unwrap();

    // 能力也被移除
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.registry().is_empty().unwrap());
}

#[test]
fn unregister_one_tier_does_not_affect_others() {
    let hub = TierHub::new();
    let l0 = TierId::new("l0");
    let l1 = TierId::new("l1");

    hub.register_tier(reg(
        "l0",
        TierLevel::L0,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();
    hub.register_tier(reg(
        "l1",
        TierLevel::L1,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();

    hub.register_workload_source(
        l0.clone(),
        100,
        Arc::new(MockWorkloadSource {
            name: "l0",
            workload: WorkloadType::Development,
        }),
    )
    .unwrap();
    hub.register_workload_source(
        l1.clone(),
        200,
        Arc::new(MockWorkloadSource {
            name: "l1",
            workload: WorkloadType::AiInference,
        }),
    )
    .unwrap();

    // 删 l0
    hub.unregister_tier(&l0).unwrap();

    // l1 仍可用
    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "l1");

    // registry 只剩 l1
    assert_eq!(hub.registry().count().unwrap(), 1);
}

#[test]
fn has_capability_reflects_registrations() {
    let hub = TierHub::new();
    hub.register_tier(reg(
        "l0",
        TierLevel::L0,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();

    assert!(hub
        .has_capability(TierCapability::WorkloadObservation)
        .unwrap());
    assert!(!hub
        .has_capability(TierCapability::NativeTaskControl)
        .unwrap());
}

#[test]
fn duplicate_tier_registration_rejected() {
    let hub = TierHub::new();
    hub.register_tier(reg(
        "dup",
        TierLevel::L0,
        vec![TierCapability::WorkloadObservation],
    ))
    .unwrap();
    assert!(hub
        .register_tier(reg(
            "dup",
            TierLevel::L0,
            vec![TierCapability::WorkloadObservation]
        ))
        .is_err());
}

// 一个空的 provider 用于测试 trait object 兼容
#[test]
fn mock_source_trait_object() {
    let src: Arc<dyn IWorkloadSource> = Arc::new(MockWorkloadSource {
        name: "test",
        workload: WorkloadType::Development,
    });
    let obs = src.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);

    // 保持 ContractError 导入被使用
    let _e: ContractError = ContractError::AiInferenceFailed("x".to_string());
}
