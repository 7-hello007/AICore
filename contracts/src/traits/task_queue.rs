use crate::errors::ContractResult;
use crate::execution::targets::ExecutionTargets;
use crate::task::{lifecycle::TaskState, queued::QueuedTask};

/// Rechecked immediately before preparation actuation. Implemented by the
/// application boundary; queue and Core never resolve OS identity themselves.
pub trait IPreparationAuthorization: Send + Sync {
    fn targets(&self) -> ContractResult<ExecutionTargets>;
}

/// Task queue abstract.
///
/// Binding:
/// - ** Do not directly access OS**.
/// - Line is clear.
/// - `dequeue` is an atomic operation; the call does not return to the same task.
pub trait ITaskQueue: Send + Sync {
    /// Enter. Return task_id assigned to the system.
    fn enqueue(&self, task: QueuedTask) -> ContractResult<String>;
    fn enqueue_preparation(
        &self,
        _task: QueuedTask,
        _ready_at_unix_ms: u64,
        _authorization: std::sync::Arc<dyn IPreparationAuthorization>,
    ) -> ContractResult<String> {
        Err(crate::errors::ContractError::NotSupported(
            "scheduled preparation queue unavailable".into(),
        ))
    }
    fn preparation_targets(&self, _task_id: &str) -> ContractResult<Option<ExecutionTargets>> {
        Ok(None)
    }

    /// Take the next highest priority Queued task, atoms set the status to Running.
    ///
    /// - Return `None` to indicate that the team is empty.
    /// - Expired task will be automatically marked Expired, not returned.
    fn dequeue(&self) -> ContractResult<Option<QueuedTask>>;

    /// Batch removes the highest priority task. The default is to call `dequeue` on a case-by-case basis to avoid the repetition of batch maintenance semantics.
    fn dequeue_batch(&self, limit: usize) -> ContractResult<Vec<QueuedTask>> {
        let mut tasks = Vec::new();
        for _ in 0..limit {
            match self.dequeue()? {
                Some(task) => tasks.push(task),
                None => break,
            }
        }
        Ok(tasks)
    }

    /// Mark the task finished.
    fn complete(&self, task_id: &str, message: Option<String>) -> ContractResult<()>;

    /// Failed to mark tab.
    fn fail(&self, task_id: &str, message: String) -> ContractResult<()>;

    /// Cancel it.
    fn cancel(&self, task_id: &str) -> ContractResult<()>;

    /// Query task status.
    fn get(&self, task_id: &str) -> ContractResult<Option<QueuedTask>>;

    /// Returns the snapshot (including terminal) of all tabs.
    fn snapshot(&self) -> ContractResult<Vec<QueuedTask>>;

    /// The number of Queued states tab in the queue.
    fn queued_count(&self) -> ContractResult<usize>;

    /// Team listing.
    fn queue_name(&self) -> &'static str;
}

/// The next state (support function) is extrapolated from the current state.
pub fn transition(current: TaskState, next: TaskState) -> ContractResult<()> {
    if current.can_transition_to(next) {
        Ok(())
    } else {
        Err(crate::errors::ContractError::InvalidRequest(format!(
            "invalid task state transition: {:?} → {:?}",
            current, next
        )))
    }
}
