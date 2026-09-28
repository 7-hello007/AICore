//! InMemoryTaskQueue unit test.

use aicore_contracts::{
    application::id::ApplicationId,
    policy::priority::PolicyPriority,
    task::{contract::TaskContract, lifecycle::TaskState, queued::QueuedTask},
    traits::task_queue::ITaskQueue,
};
use aicore_task_runtime::InMemoryTaskQueue;

fn task(id: &str, priority: Option<PolicyPriority>, deadline: Option<u64>) -> QueuedTask {
    let mut c = TaskContract::minimal(ApplicationId::new("com.example.app"));
    c.task_id = Some(id.to_string());
    c.priority = priority;
    c.deadline_unix_secs = deadline;
    QueuedTask::new(id.to_string(), c, 1000)
}

#[test]
fn queue_starts_empty() {
    let q = InMemoryTaskQueue::new();
    assert_eq!(q.queued_count().unwrap(), 0);
    assert!(q.dequeue().unwrap().is_none());
    assert_eq!(q.queue_name(), "in_memory_task_queue");
}

#[test]
fn enqueue_and_dequeue() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    assert_eq!(q.queued_count().unwrap(), 1);

    let got = q.dequeue().unwrap().unwrap();
    assert_eq!(got.task_id, "t1");
    assert_eq!(got.state, TaskState::Running);
    assert_eq!(q.queued_count().unwrap(), 0);
}

#[test]
fn duplicate_task_id_rejected() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    assert!(q.enqueue(task("t1", None, None)).is_err());
}

#[test]
fn empty_task_id_gets_assigned() {
    let q = InMemoryTaskQueue::new();
    let mut t = task("", None, None);
    t.task_id = String::new();
    let id = q.enqueue(t).unwrap();
    assert!(id.starts_with("task-"));
}

#[test]
fn priority_ordering() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("low", Some(PolicyPriority::Low), None))
        .unwrap();
    q.enqueue(task("high", Some(PolicyPriority::High), None))
        .unwrap();
    q.enqueue(task("normal", Some(PolicyPriority::Normal), None))
        .unwrap();

    let first = q.dequeue().unwrap().unwrap();
    assert_eq!(first.task_id, "high");

    let second = q.dequeue().unwrap().unwrap();
    assert_eq!(second.task_id, "normal");

    let third = q.dequeue().unwrap().unwrap();
    assert_eq!(third.task_id, "low");
}

#[test]
fn deadline_overrides_priority() {
    let q = InMemoryTaskQueue::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Low priority but deadline is close.
    q.enqueue(task("urgent", Some(PolicyPriority::Low), Some(now + 5)))
        .unwrap();
    // High Priority No Deadline
    q.enqueue(task("high", Some(PolicyPriority::High), None))
        .unwrap();

    let first = q.dequeue().unwrap().unwrap();
    assert_eq!(first.task_id, "urgent");
}

#[test]
fn expired_task_is_marked_expired_before_dequeue() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("expired", None, Some(500))).unwrap(); // Deadline is past.

    let got = q.dequeue().unwrap();
    assert!(got.is_none());

    let snapshot = q.snapshot().unwrap();
    assert!(snapshot
        .iter()
        .any(|t| t.task_id == "expired" && t.state == TaskState::Expired));
    assert_eq!(q.queued_count().unwrap(), 0);
}

#[test]
fn complete_transitions_to_completed() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    let _ = q.dequeue().unwrap().unwrap();

    q.complete("t1", Some("done".to_string())).unwrap();
    let t = q.get("t1").unwrap().unwrap();
    assert_eq!(t.state, TaskState::Completed);
    assert_eq!(t.result_message.as_deref(), Some("done"));
    assert!(t.finished_at_unix_ms.is_some());
}

#[test]
fn fail_transitions_to_failed() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    let _ = q.dequeue().unwrap().unwrap();
    q.fail("t1", "boom".to_string()).unwrap();
    assert_eq!(q.get("t1").unwrap().unwrap().state, TaskState::Failed);
}

#[test]
fn cancel_from_queued() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    q.cancel("t1").unwrap();
    assert_eq!(q.get("t1").unwrap().unwrap().state, TaskState::Cancelled);
}

#[test]
fn invalid_transition_rejected() {
    let q = InMemoryTaskQueue::new();
    q.enqueue(task("t1", None, None)).unwrap();
    // It's illegal.
    assert!(q.complete("t1", None).is_err());
}

#[test]
fn complete_unknown_task_fails() {
    let q = InMemoryTaskQueue::new();
    assert!(q.complete("nonexistent", None).is_err());
}
