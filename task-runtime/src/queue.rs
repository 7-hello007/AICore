use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    task::{lifecycle::TaskState, queued::QueuedTask},
    traits::task_queue::{transition, IPreparationAuthorization, ITaskQueue},
};

/// Memory Task queue.
///
/// - Priority: `effective_priority` (including deadline).
/// - Expiry: check deadline at `dequeue`, expired Expired.
/// - Thread security: internal `Mutex<QueueState>`.
pub struct InMemoryTaskQueue {
    state: Mutex<QueueState>,
}

#[derive(Default)]
struct QueueState {
    /// task_id → QueuedTask
    tasks: HashMap<String, QueuedTask>,
    /// Order id counter (used to generate task_id).
    next_seq: AtomicU64,
    preparations: HashMap<String, (u64, Arc<dyn IPreparationAuthorization>)>,
}

impl InMemoryTaskQueue {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(QueueState::default()),
        }
    }

    fn lock(&self) -> ContractResult<std::sync::MutexGuard<'_, QueueState>> {
        self.state
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("task queue lock poisoned".into()))
    }
}

impl Default for InMemoryTaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ITaskQueue for InMemoryTaskQueue {
    fn enqueue(&self, task: QueuedTask) -> ContractResult<String> {
        let mut state = self.lock()?;
        insert_task(&mut state, task)
    }
    fn enqueue_preparation(
        &self,
        task: QueuedTask,
        ready_at_unix_ms: u64,
        authorization: Arc<dyn IPreparationAuthorization>,
    ) -> ContractResult<String> {
        if task.contract.deadline_unix_secs.is_none()
            || task
                .contract
                .deadline_unix_secs
                .is_some_and(|deadline| ready_at_unix_ms >= deadline.saturating_mul(1000))
        {
            return Err(ContractError::InvalidRequest(
                "preparation requires a ready time before expiry".into(),
            ));
        }
        let mut state = self.lock()?;
        let id = insert_task(&mut state, task)?;
        state
            .preparations
            .insert(id.clone(), (ready_at_unix_ms, authorization));
        Ok(id)
    }
    fn preparation_targets(
        &self,
        task_id: &str,
    ) -> ContractResult<Option<aicore_contracts::execution::targets::ExecutionTargets>> {
        let authorization = self
            .lock()?
            .preparations
            .get(task_id)
            .map(|(_, guard)| guard.clone());
        authorization.map(|guard| guard.targets()).transpose()
    }

    fn dequeue(&self) -> ContractResult<Option<QueuedTask>> {
        let mut state = self.lock()?;
        let now_secs = now_unix_secs();
        let now_ms = now_unix_ms();

        // 1. Expired clean-up: all Queued and expired Expired
        let expired_ids: Vec<String> = state
            .tasks
            .values()
            .filter(|t| t.is_expired(now_secs))
            .map(|t| t.task_id.clone())
            .collect();
        for id in expired_ids {
            if let Some(task) = state.tasks.get_mut(&id) {
                transition(task.state, TaskState::Expired)?;
                task.state = TaskState::Expired;
                task.finished_at_unix_ms = Some(now_ms);
                task.result_message = Some("deadline expired before execution".to_string());
            }
            state.preparations.remove(&id);
        }

        // 2. Finding the highest priority
        let candidate = state
            .tasks
            .values()
            .filter(|t| t.state == TaskState::Queued)
            .filter(|t| {
                state
                    .preparations
                    .get(&t.task_id)
                    .map(|(ready, _)| *ready <= now_ms)
                    .unwrap_or(true)
            })
            .max_by_key(|t| t.effective_priority(now_secs))
            .map(|t| t.task_id.clone());

        let id = match candidate {
            Some(id) => id,
            None => return Ok(None),
        };

        let task = state.tasks.get_mut(&id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!("task disappeared before dequeue: {}", id))
        })?;
        transition(task.state, TaskState::Running)?;
        task.state = TaskState::Running;
        task.started_at_unix_ms = Some(now_ms);

        Ok(Some(task.clone()))
    }

    fn complete(&self, task_id: &str, message: Option<String>) -> ContractResult<()> {
        self.finish(task_id, TaskState::Completed, message)
    }

    fn fail(&self, task_id: &str, message: String) -> ContractResult<()> {
        self.finish(task_id, TaskState::Failed, Some(message))
    }

    fn cancel(&self, task_id: &str) -> ContractResult<()> {
        self.finish(task_id, TaskState::Cancelled, None)
    }

    fn get(&self, task_id: &str) -> ContractResult<Option<QueuedTask>> {
        let state = self.lock()?;
        Ok(state.tasks.get(task_id).cloned())
    }

    fn snapshot(&self) -> ContractResult<Vec<QueuedTask>> {
        let state = self.lock()?;
        let mut out: Vec<QueuedTask> = state.tasks.values().cloned().collect();
        out.sort_by_key(|t| t.enqueued_at_unix_ms);
        Ok(out)
    }

    fn queued_count(&self) -> ContractResult<usize> {
        let state = self.lock()?;
        Ok(state
            .tasks
            .values()
            .filter(|t| t.state == TaskState::Queued)
            .count())
    }

    fn queue_name(&self) -> &'static str {
        "in_memory_task_queue"
    }
}

impl InMemoryTaskQueue {
    /// Cancel all tasks that remain queued/running.
    pub fn cancel_all(&self) -> ContractResult<usize> {
        let mut state = self.lock()?;
        let now = now_unix_ms();
        let ids: Vec<String> = state
            .tasks
            .values()
            .filter(|t| t.state.is_active())
            .map(|t| t.task_id.clone())
            .collect();
        let mut count = 0usize;
        for id in ids {
            if let Some(task) = state.tasks.get_mut(&id) {
                if task.state.can_transition_to(TaskState::Cancelled) {
                    task.state = TaskState::Cancelled;
                    task.finished_at_unix_ms = Some(now);
                    task.result_message = Some("cancelled by queue shutdown".to_string());
                    count += 1;
                }
            }
            state.preparations.remove(&id);
        }
        Ok(count)
    }

    /// Clears all tasks. This is mainly for testing and explicit user operations.
    pub fn clear(&self) -> ContractResult<()> {
        let mut state = self.lock()?;
        state.tasks.clear();
        state.preparations.clear();
        Ok(())
    }

    fn finish(
        &self,
        task_id: &str,
        target: TaskState,
        message: Option<String>,
    ) -> ContractResult<()> {
        let mut state = self.lock()?;
        let now = now_unix_ms();
        let task = state.tasks.get_mut(task_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!("task not found: {}", task_id))
        })?;

        transition(task.state, target)?;
        task.state = target;
        task.finished_at_unix_ms = Some(now);
        task.result_message = message;
        state.preparations.remove(task_id);
        Ok(())
    }
}

fn insert_task(state: &mut QueueState, mut task: QueuedTask) -> ContractResult<String> {
    if task.task_id.trim().is_empty() {
        let seq = state.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
        task.task_id = format!("task-{}-{}", now_unix_secs(), seq);
    }
    let id = task.task_id.clone();
    if state.tasks.contains_key(&id) {
        return Err(ContractError::InvalidRequest(format!(
            "duplicate task_id: {id}"
        )));
    }
    state.tasks.insert(id.clone(), task);
    Ok(id)
}

/// Share packaging to facilitate simultaneous use of daemon and AdaptiveLoop.
pub type SharedTaskQueue = Arc<dyn ITaskQueue>;

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    use aicore_contracts::{
        application::id::ApplicationId, execution::targets::ExecutionTargets,
        task::contract::TaskContract,
    };
    struct Guard;
    impl IPreparationAuthorization for Guard {
        fn targets(&self) -> ContractResult<ExecutionTargets> {
            Ok(ExecutionTargets {
                process_only: true,
                foreground_pids: vec![42],
                background_pids: vec![],
            })
        }
    }
    #[test]
    fn delayed_preparation_cancellation_expiry_release_authorization() {
        let q = InMemoryTaskQueue::new();
        let make = |id: &str, deadline| {
            let mut contract = TaskContract::minimal(ApplicationId::new("app.test"));
            contract.deadline_unix_secs = Some(deadline);
            QueuedTask::new(id.into(), contract, now_unix_ms())
        };
        let guard = Arc::new(Guard);
        q.enqueue_preparation(
            make("later", now_unix_secs() + 60),
            now_unix_ms() + 30_000,
            guard.clone(),
        )
        .unwrap();
        assert!(q.dequeue().unwrap().is_none());
        q.cancel("later").unwrap();
        assert_eq!(q.get("later").unwrap().unwrap().state, TaskState::Cancelled);
        assert!(q.preparation_targets("later").unwrap().is_none());
        q.enqueue_preparation(make("expired", 1), 0, guard.clone())
            .unwrap();
        assert!(q.dequeue().unwrap().is_none());
        assert_eq!(q.get("expired").unwrap().unwrap().state, TaskState::Expired);
        assert!(q.preparation_targets("expired").unwrap().is_none());
        q.enqueue_preparation(
            make("ready", now_unix_secs() + 60),
            now_unix_ms(),
            guard.clone(),
        )
        .unwrap();
        assert_eq!(q.dequeue().unwrap().unwrap().task_id, "ready");
        assert!(
            q.preparation_targets("ready")
                .unwrap()
                .unwrap()
                .process_only
        );
        assert_eq!(q.cancel_all().unwrap(), 1);
        assert!(q.preparation_targets("ready").unwrap().is_none());
        assert_eq!(Arc::strong_count(&guard), 1);
    }
}
