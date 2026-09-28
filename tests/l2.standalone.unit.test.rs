//! L2 Independent testing.

use aicore_contracts::{
    application::id::ApplicationId,
    context::workload::WorkloadType,
    task::contract::TaskContract,
    task::lifecycle::TaskState,
    task::queued::QueuedTask,
    tier::capability::TierCapability,
    traits::{
        native_task_provider::{INativeTaskProvider, NativeTaskHandle},
        task_queue::ITaskQueue,
    },
};
use aicore_task_runtime::InMemoryTaskQueue;
use aicore_tier::TierHub;
use aicore_tier_l2::{register_l2, register_l2_with_queue, L2_TIER_ID};
use std::sync::Arc;

fn ai_contract() -> TaskContract {
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.ai"));
    c.task_id = Some("ai-1".to_string());
    c.task = Some(WorkloadType::AiInference);
    c
}

#[test]
fn l2_registers_and_provides_provider() {
    let hub = TierHub::new();
    let (id, _provider) = register_l2(&hub).unwrap();
    assert_eq!(id.as_str(), L2_TIER_ID);

    assert!(hub.best_native_task_provider().unwrap().is_some());
    assert_eq!(hub.registry().count().unwrap(), 1);
}

#[test]
fn l2_submit_returns_handle() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();

    let handle = provider.submit(&ai_contract()).unwrap();
    assert!(handle.handle_id.starts_with("native-"));
    assert_eq!(provider.task_count(), 1);
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Queued);
}

#[test]
fn l2_state_machine() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();
    let handle = provider.submit(&ai_contract()).unwrap();

    provider.transition(&handle, TaskState::Running).unwrap();
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Running);

    provider.transition(&handle, TaskState::Completed).unwrap();
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Completed);
}

#[test]
fn l2_invalid_transition_rejected() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();
    let handle = provider.submit(&ai_contract()).unwrap();

    // Pending Completed Illegal
    assert!(provider.transition(&handle, TaskState::Completed).is_err());
}

#[test]
fn l2_cancel_pending_works() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();
    let handle = provider.submit(&ai_contract()).unwrap();

    provider.cancel(&handle).unwrap();
    assert_eq!(provider.query(&handle).unwrap(), TaskState::Cancelled);
}

#[test]
fn l2_cancel_completed_rejected() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();
    let handle = provider.submit(&ai_contract()).unwrap();

    provider.transition(&handle, TaskState::Running).unwrap();
    provider.transition(&handle, TaskState::Completed).unwrap();

    assert!(provider.cancel(&handle).is_err());
}

#[test]
fn l2_query_unknown_handle_fails() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();

    let fake = NativeTaskHandle {
        handle_id: "nonexistent".to_string(),
    };
    assert!(provider.query(&fake).is_err());
}

#[test]
fn queue_backed_l2_only_controls_its_own_handles() {
    let queue = Arc::new(InMemoryTaskQueue::new());
    queue
        .enqueue(QueuedTask::new(
            "ordinary-task".to_string(),
            ai_contract(),
            1,
        ))
        .unwrap();
    let hub = TierHub::new();
    let (_, provider) = register_l2_with_queue(&hub, queue.clone()).unwrap();
    let ordinary = NativeTaskHandle {
        handle_id: "ordinary-task".to_string(),
    };
    assert!(provider.query(&ordinary).is_err());
    assert!(provider.cancel(&ordinary).is_err());
    assert_eq!(
        queue.get("ordinary-task").unwrap().unwrap().state,
        TaskState::Queued
    );

    let native = provider.submit(&ai_contract()).unwrap();
    assert_eq!(provider.query(&native).unwrap(), TaskState::Queued);
    provider.cancel(&native).unwrap();
    assert_eq!(provider.query(&native).unwrap(), TaskState::Cancelled);
}

#[test]
fn l2_source_metadata() {
    let hub = TierHub::new();
    let (_id, provider) = register_l2(&hub).unwrap();
    assert_eq!(provider.source_name(), "l2.native_task_provider");
}

#[test]
fn l2_declares_native_control() {
    let hub = TierHub::new();
    register_l2(&hub).unwrap();
    assert!(hub
        .has_capability(TierCapability::NativeTaskControl)
        .unwrap());
}

#[test]
fn l2_duplicate_registration_rejected() {
    let hub = TierHub::new();
    register_l2(&hub).unwrap();
    assert!(register_l2(&hub).is_err());
}

#[test]
fn l2_removal_leaves_empty_hub_valid() {
    let hub = TierHub::new();
    let (id, _) = register_l2(&hub).unwrap();

    hub.unregister_tier(&id).unwrap();
    assert!(hub.best_native_task_provider().unwrap().is_none());
    assert!(hub.registry().is_empty().unwrap());
}
