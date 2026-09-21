//! Tier removal 测试。
//!
//! 核心不变量：
//! **删除任一 Tier，其他 Tier 与 Core 均不受影响。**

use std::sync::Arc;

use aicore_contracts::context::workload::WorkloadType;
use aicore_contracts::{
    errors::ContractResult,
    telemetry::snapshot::ComputerSnapshot,
    tier::{
        capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
    },
    traits::workload_source::{IWorkloadSource, WorkloadObservation},
};
use aicore_tier::TierHub;

struct L0Source;
impl IWorkloadSource for L0Source {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        Ok(WorkloadObservation {
            workload_type: WorkloadType::Gaming,
            confidence: 0.7,
            evidence: vec!["l0 inference".to_string()],
        })
    }
    fn source_name(&self) -> &'static str {
        "l0"
    }
}

struct L1Source;
impl IWorkloadSource for L1Source {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        Ok(WorkloadObservation {
            workload_type: WorkloadType::Development,
            confidence: 0.95,
            evidence: vec!["l1 declaration".to_string()],
        })
    }
    fn source_name(&self) -> &'static str {
        "l1"
    }
}

struct L2Source;
impl IWorkloadSource for L2Source {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        Ok(WorkloadObservation {
            workload_type: WorkloadType::AiInference,
            confidence: 1.0,
            evidence: vec!["l2 native".to_string()],
        })
    }
    fn source_name(&self) -> &'static str {
        "l2"
    }
}

fn reg(id: &str, level: TierLevel) -> TierRegistration {
    TierRegistration::new(TierId::new(id), level, "1.0.0")
        .with_capability(TierCapability::WorkloadObservation)
}

fn build_all(hub: &TierHub) -> (TierId, TierId, TierId) {
    let l0 = TierId::new("l0");
    let l1 = TierId::new("l1");
    let l2 = TierId::new("l2");

    hub.register_tier(reg("l0", TierLevel::L0)).unwrap();
    hub.register_tier(reg("l1", TierLevel::L1)).unwrap();
    hub.register_tier(reg("l2", TierLevel::L2)).unwrap();

    hub.register_workload_source(l0.clone(), 100, Arc::new(L0Source))
        .unwrap();
    hub.register_workload_source(l1.clone(), 200, Arc::new(L1Source))
        .unwrap();
    hub.register_workload_source(l2.clone(), 300, Arc::new(L2Source))
        .unwrap();

    (l0, l1, l2)
}

#[test]
fn remove_l0_keeps_l1_l2() {
    let hub = TierHub::new();
    let (l0, _l1, _l2) = build_all(&hub);

    assert_eq!(hub.registry().count().unwrap(), 3);
    assert_eq!(hub.all_workload_sources().unwrap().len(), 3);

    hub.unregister_tier(&l0).unwrap();

    assert_eq!(hub.registry().count().unwrap(), 2);
    assert_eq!(hub.all_workload_sources().unwrap().len(), 2);

    // l2 优先级最高，仍是 best
    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "l2");
}

#[test]
fn remove_l1_keeps_l0_l2() {
    let hub = TierHub::new();
    let (_l0, l1, _l2) = build_all(&hub);

    hub.unregister_tier(&l1).unwrap();

    assert_eq!(hub.registry().count().unwrap(), 2);
    let all = hub.all_workload_sources().unwrap();
    assert_eq!(all.len(), 2);

    // 剩下的按优先级：l2 (300) > l0 (100)
    assert_eq!(all[0].source_name(), "l2");
    assert_eq!(all[1].source_name(), "l0");
}

#[test]
fn remove_l2_keeps_l0_l1() {
    let hub = TierHub::new();
    let (_l0, _l1, l2) = build_all(&hub);

    hub.unregister_tier(&l2).unwrap();

    assert_eq!(hub.registry().count().unwrap(), 2);

    // 现在 l1 是 best（200 > 100）
    let best = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(best.source_name(), "l1");
}

#[test]
fn remove_all_tiers_leaves_empty_but_valid() {
    let hub = TierHub::new();
    let (l0, l1, l2) = build_all(&hub);

    hub.unregister_tier(&l0).unwrap();
    hub.unregister_tier(&l1).unwrap();
    hub.unregister_tier(&l2).unwrap();

    // 空状态合法
    assert!(hub.registry().is_empty().unwrap());
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.all_workload_sources().unwrap().is_empty());
}

#[test]
fn re_register_after_removal_works() {
    let hub = TierHub::new();
    let (l0, _, _) = build_all(&hub);

    hub.unregister_tier(&l0).unwrap();
    // 重新注册同名 Tier 应可成功
    hub.register_tier(reg("l0", TierLevel::L0)).unwrap();
    hub.register_workload_source(l0.clone(), 100, Arc::new(L0Source))
        .unwrap();

    assert_eq!(hub.registry().count().unwrap(), 3);
    // 三个都有 workload source
    assert_eq!(hub.all_workload_sources().unwrap().len(), 3);
}
