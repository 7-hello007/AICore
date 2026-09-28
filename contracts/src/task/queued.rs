use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;

use super::{contract::TaskContract, lifecycle::TaskState};

/// Queue the task entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueuedTask {
    /// The system allocates task_id (if not provided by contract, create a queue).
    pub task_id: String,
    pub application_id: ApplicationId,
    pub contract: TaskContract,
    pub state: TaskState,
    pub enqueued_at_unix_ms: u64,
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
    /// Finish/ Failed Reason.
    pub result_message: Option<String>,
}

impl QueuedTask {
    pub fn new(task_id: String, contract: TaskContract, now: u64) -> Self {
        Self {
            task_id,
            application_id: contract.application_id.clone(),
            contract,
            state: TaskState::Queued,
            enqueued_at_unix_ms: now,
            started_at_unix_ms: None,
            finished_at_unix_ms: None,
            result_message: None,
        }
    }

    /// Expiry.
    pub fn is_expired(&self, now: u64) -> bool {
        if let Some(deadline) = self.contract.deadline_unix_secs {
            now >= deadline && self.state == TaskState::Queued
        } else {
            false
        }
    }

    /// Effective priority: the closer the deadline gets.
    ///
    /// Returns a higher priority value.
    pub fn effective_priority(&self, now: u64) -> i64 {
        use crate::policy::priority::PolicyPriority;
        let base: i64 = match self.contract.priority {
            Some(PolicyPriority::Interactive) => 1000,
            Some(PolicyPriority::High) => 800,
            Some(PolicyPriority::Normal) => 500,
            Some(PolicyPriority::Low) => 300,
            Some(PolicyPriority::Background) => 100,
            None => 400,
        };

        // The closer the deadline, the more points; the upper 1000.
        //
        // Semantics: A significant addition to the task within 1 hour of the deadline;
        // The closer you add, the bigger you get, the more you cover the base priority difference.
        if let Some(deadline) = self.contract.deadline_unix_secs {
            if deadline > now {
                let time_to_deadline = (deadline - now) as i64;
                let bonus = (3600_i64 - time_to_deadline).clamp(0, 1000);
                base + bonus
            } else {
                // Expire: negative score, next dequee
                base - 1000
            }
        } else {
            base
        }
    }
}
