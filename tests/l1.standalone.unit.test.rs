//! L1 独立测试。

use aicore_contracts::{
    application::id::ApplicationId, context::workload::WorkloadType, task::contract::TaskContract,
    tier::capability::TierCapability, traits::task_source::ITaskSource,
};
use aicore_tier::TierHub;
use aicore_tier_l1::{register_l1, L1_TIER_ID};

fn dev_contract() -> TaskContract {
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.ide"));
    c.task_id = Some("t1".to_string());
    c.task = Some(WorkloadType::Development);
    c
}

#[test]
fn l1_registers_and_provides_source() {
    let hub = TierHub::new();
    let (id, _source) = register_l1(&hub).unwrap();
    assert_eq!(id.as_str(), L1_TIER_ID);

    assert!(hub.best_task_source().unwrap().is_some());
    assert_eq!(hub.registry().count().unwrap(), 1);
}

#[test]
fn l1_task_source_initially_empty() {
    let hub = TierHub::new();
    let (_id, source) = register_l1(&hub).unwrap();
    assert!(!source.has_current());
    assert!(source.current_task().unwrap().is_none());
}

#[test]
fn l1_set_and_read_current_task() {
    let hub = TierHub::new();
    let (_id, source) = register_l1(&hub).unwrap();

    source.set_current(dev_contract()).unwrap();
    assert!(source.has_current());

    let got = source.current_task().unwrap().unwrap();
    assert_eq!(got.task_id.as_deref(), Some("t1"));
    assert_eq!(got.task, Some(WorkloadType::Development));
}

#[test]
fn l1_clear_removes_current() {
    let hub = TierHub::new();
    let (_id, source) = register_l1(&hub).unwrap();

    source.set_current(dev_contract()).unwrap();
    source.clear().unwrap();
    assert!(!source.has_current());
    assert!(source.current_task().unwrap().is_none());
}

#[test]
fn l1_source_metadata() {
    let hub = TierHub::new();
    let (_id, source) = register_l1(&hub).unwrap();
    assert_eq!(source.source_name(), "l1.task_source");
    assert_eq!(source.priority(), 500);
}

#[test]
fn l1_declares_expected_capabilities() {
    let hub = TierHub::new();
    register_l1(&hub).unwrap();

    assert!(hub.has_capability(TierCapability::TaskMetadata).unwrap());
    assert!(hub.has_capability(TierCapability::ResourceHints).unwrap());
    assert!(!hub
        .has_capability(TierCapability::NativeTaskControl)
        .unwrap());
}

#[test]
fn l1_duplicate_registration_rejected() {
    let hub = TierHub::new();
    register_l1(&hub).unwrap();
    assert!(register_l1(&hub).is_err());
}

#[test]
fn l1_removal_leaves_empty_hub_valid() {
    let hub = TierHub::new();
    let (id, _) = register_l1(&hub).unwrap();

    hub.unregister_tier(&id).unwrap();
    assert!(hub.best_task_source().unwrap().is_none());
    assert!(hub.registry().is_empty().unwrap());
}
