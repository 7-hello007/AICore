//! TaskState status machine test.

use aicore_contracts::task::lifecycle::TaskState;

#[test]
fn terminal_states() {
    assert!(TaskState::Completed.is_terminal());
    assert!(TaskState::Failed.is_terminal());
    assert!(TaskState::Cancelled.is_terminal());
    assert!(TaskState::Expired.is_terminal());
    assert!(!TaskState::Queued.is_terminal());
    assert!(!TaskState::Running.is_terminal());
}

#[test]
fn active_states() {
    assert!(TaskState::Queued.is_active());
    assert!(TaskState::Running.is_active());
    assert!(!TaskState::Completed.is_active());
}

#[test]
fn valid_transitions() {
    assert!(TaskState::Queued.can_transition_to(TaskState::Running));
    assert!(TaskState::Queued.can_transition_to(TaskState::Cancelled));
    assert!(TaskState::Queued.can_transition_to(TaskState::Expired));
    assert!(TaskState::Running.can_transition_to(TaskState::Completed));
    assert!(TaskState::Running.can_transition_to(TaskState::Failed));
    assert!(TaskState::Running.can_transition_to(TaskState::Cancelled));
}

#[test]
fn invalid_transitions() {
    assert!(!TaskState::Queued.can_transition_to(TaskState::Completed));
    assert!(!TaskState::Completed.can_transition_to(TaskState::Running));
    assert!(!TaskState::Failed.can_transition_to(TaskState::Running));
    assert!(!TaskState::Running.can_transition_to(TaskState::Queued));
}

#[test]
fn serde_roundtrip() {
    for s in [
        TaskState::Queued,
        TaskState::Running,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Cancelled,
        TaskState::Expired,
    ] {
        let json = serde_json::to_string(&s).unwrap();
        let back: TaskState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
