//! 多 Tier 隔离测试。
//!
//! 核心不变量：
//! **删除 L0 / L1 / L2 中任意一个，其他 Tier 与 Core 均不受影响。**

use aicore_contracts::errors::AicoreError;
use aicore_contracts::{
    application::id::ApplicationId, context::workload::WorkloadType, task::contract::TaskContract,
    task::lifecycle::TaskState, traits::native_task_provider::INativeTaskProvider,
};
use aicore_tier::TierHub;
use aicore_tier_l0::register_l0;
use aicore_tier_l1::register_l1;
use aicore_tier_l2::register_l2;

fn dev_contract() -> TaskContract {
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.ide"));
    c.task_id = Some("dev-1".to_string());
    c.task = Some(WorkloadType::Development);
    c
}

#[test]
fn all_three_tiers_coexist() {
    let hub = TierHub::new();
    let l0 = register_l0(&hub).unwrap();
    let (l1, _) = register_l1(&hub).unwrap();
    let (l2, _) = register_l2(&hub).unwrap();

    assert_eq!(l0.as_str(), "l0.legacy");
    assert_eq!(l1.as_str(), "l1.aware");
    assert_eq!(l2.as_str(), "l2.native");
    assert_eq!(hub.registry().count().unwrap(), 3);

    assert!(hub.best_workload_source().unwrap().is_some());
    assert!(hub.best_task_source().unwrap().is_some());
    assert!(hub.best_native_task_provider().unwrap().is_some());
}

#[test]
fn remove_l0_keeps_l1_l2() {
    let hub = TierHub::new();
    let l0 = register_l0(&hub).unwrap();
    register_l1(&hub).unwrap();
    register_l2(&hub).unwrap();

    hub.unregister_tier(&l0).unwrap();

    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.best_task_source().unwrap().is_some());
    assert!(hub.best_native_task_provider().unwrap().is_some());
    assert_eq!(hub.registry().count().unwrap(), 2);
}

#[test]
fn remove_l1_keeps_l0_l2() {
    let hub = TierHub::new();
    register_l0(&hub).unwrap();
    let (l1, _) = register_l1(&hub).unwrap();
    register_l2(&hub).unwrap();

    hub.unregister_tier(&l1).unwrap();

    assert!(hub.best_workload_source().unwrap().is_some());
    assert!(hub.best_task_source().unwrap().is_none());
    assert!(hub.best_native_task_provider().unwrap().is_some());
    assert_eq!(hub.registry().count().unwrap(), 2);
}

#[test]
fn remove_l2_keeps_l0_l1() {
    let hub = TierHub::new();
    register_l0(&hub).unwrap();
    register_l1(&hub).unwrap();
    let (l2, _) = register_l2(&hub).unwrap();

    hub.unregister_tier(&l2).unwrap();

    assert!(hub.best_workload_source().unwrap().is_some());
    assert!(hub.best_task_source().unwrap().is_some());
    assert!(hub.best_native_task_provider().unwrap().is_none());
    assert_eq!(hub.registry().count().unwrap(), 2);
}

#[test]
fn l1_declaration_flows_through_task_source() {
    let hub = TierHub::new();
    let (_l1, source) = register_l1(&hub).unwrap();

    source.set_current(dev_contract()).unwrap();

    // 通过 hub 获取 task source（trait object；无需 ITaskSource 在 scope）
    let best = hub.best_task_source().unwrap().unwrap();
    let got = best.current_task().unwrap().unwrap();
    assert_eq!(got.task, Some(WorkloadType::Development));
}

#[test]
fn l2_native_task_lifecycle_end_to_end() {
    let hub = TierHub::new();
    let (_l2, provider) = register_l2(&hub).unwrap();

    let handle = provider.submit(&dev_contract()).unwrap();
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Queued);

    provider.transition(&handle, TaskState::Running).unwrap();
    provider.transition(&handle, TaskState::Completed).unwrap();
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Completed);
}

#[test]
fn all_tiers_removed_leaves_empty_but_valid() {
    let hub = TierHub::new();
    let l0 = register_l0(&hub).unwrap();
    let (l1, _) = register_l1(&hub).unwrap();
    let (l2, _) = register_l2(&hub).unwrap();

    hub.unregister_tier(&l0).unwrap();
    hub.unregister_tier(&l1).unwrap();
    hub.unregister_tier(&l2).unwrap();

    assert!(hub.registry().is_empty().unwrap());
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.best_task_source().unwrap().is_none());
    assert!(hub.best_native_task_provider().unwrap().is_none());
}

#[test]
fn tier_l0_cannot_declare_native_control() {
    use aicore_contracts::tier::{
        capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
    };

    let hub = TierHub::new();
    let bad = TierRegistration::new(TierId::new("bad.l0"), TierLevel::L0, "1.0.0")
        .with_capability(TierCapability::NativeTaskControl);
    let err = hub.register_tier(bad).unwrap_err();
    match err {
        aicore_tier::RegistryError::InvalidRegistration(
            AicoreError::TierLevelCannotControlNative(_),
        ) => {},
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn tier_l1_cannot_declare_native_control() {
    use aicore_contracts::tier::{
        capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
    };

    let hub = TierHub::new();
    let bad = TierRegistration::new(TierId::new("bad.l1"), TierLevel::L1, "1.0.0")
        .with_capability(TierCapability::NativeTaskControl);
    let err = hub.register_tier(bad).unwrap_err();
    match err {
        aicore_tier::RegistryError::InvalidRegistration(
            AicoreError::TierLevelCannotControlNative(_),
        ) => {},
        other => panic!("unexpected error: {:?}", other),
    }
}
