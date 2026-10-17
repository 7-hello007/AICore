use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    task::contract::TaskContract,
    task::lifecycle::TaskState,
    task::queued::QueuedTask,
    traits::native_task_provider::{INativeTaskProvider, NativeTaskHandle},
    traits::task_queue::ITaskQueue,
};

/// Internal records.
struct NativeTaskRecord {
    contract: TaskContract,
    state: TaskState,
    submitted_at_unix_secs: u64,
}

/// L2 Origin task control provider.
///
/// Semantics:
/// - Apply `submit(contract)` for original tasks; return handle.
/// - Through `query(handle)` query status.
/// - Request cancelled through `cancel(handle)`.
/// - The `transition(handle, state)` in a stand-alone mode is a Core driver; it is driven by a queue when the sharing queue is bound.
///
/// ** Binding**:
/// - L2 does not directly perform OS operations; only manage task metadata.
/// - The actual execution still follows the Validator→Executor path of Core.
pub struct L2NativeTaskProvider {
    tasks: Mutex<HashMap<String, NativeTaskRecord>>,
    next_seq: AtomicU64,
    queue: Option<Arc<dyn ITaskQueue>>,
}

impl L2NativeTaskProvider {
    pub fn try_submit(&self, contract: &TaskContract) -> ContractResult<NativeTaskHandle> {
        let mut tasks = self
            .tasks
            .try_lock()
            .map_err(|_| ContractError::ResourceUnavailable("L2 tasks busy".into()))?;
        let handle_id = format!(
            "native-{}-{}",
            now_unix_secs(),
            self.next_seq.fetch_add(1, Ordering::SeqCst)
        );
        if let Some(queue) = &self.queue {
            queue.try_enqueue(QueuedTask::new(
                handle_id.clone(),
                contract.clone(),
                now_unix_ms(),
            ))?;
        }
        tasks.insert(
            handle_id.clone(),
            NativeTaskRecord {
                contract: contract.clone(),
                state: TaskState::Queued,
                submitted_at_unix_secs: now_unix_secs(),
            },
        );
        Ok(NativeTaskHandle { handle_id })
    }
    pub fn try_task_info(
        &self,
        handle: &NativeTaskHandle,
    ) -> ContractResult<(TaskContract, u64, TaskState)> {
        let tasks = self
            .tasks
            .try_lock()
            .map_err(|_| ContractError::ResourceUnavailable("L2 tasks busy".into()))?;
        let record = tasks
            .get(&handle.handle_id)
            .ok_or_else(|| ContractError::ResourceUnavailable("native task not found".into()))?;
        let state = if let Some(queue) = &self.queue {
            queue
                .try_get(&handle.handle_id)?
                .ok_or_else(|| {
                    ContractError::ResourceUnavailable("queued native task missing".into())
                })?
                .state
        } else {
            record.state
        };
        Ok((
            record.contract.clone(),
            record.submitted_at_unix_secs,
            state,
        ))
    }
    pub fn try_cancel(&self, handle: &NativeTaskHandle) -> ContractResult<()> {
        let mut tasks = self
            .tasks
            .try_lock()
            .map_err(|_| ContractError::ResourceUnavailable("L2 tasks busy".into()))?;
        let record = tasks
            .get_mut(&handle.handle_id)
            .ok_or_else(|| ContractError::ResourceUnavailable("native task not found".into()))?;
        if let Some(queue) = &self.queue {
            queue.try_cancel(&handle.handle_id)
        } else {
            aicore_contracts::traits::task_queue::transition(record.state, TaskState::Cancelled)?;
            record.state = TaskState::Cancelled;
            Ok(())
        }
    }
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            next_seq: AtomicU64::new(1),
            queue: None,
        }
    }

    /// Bind native handles to the daemon's existing task queue.
    pub fn with_task_queue(mut self, queue: Arc<dyn ITaskQueue>) -> Self {
        self.queue = Some(queue);
        self
    }

    /// Status conversion. Allow:
    /// - Pending → Running / Cancelled / Failed
    /// - Running → Completed / Failed / Cancelled
    pub fn transition(&self, handle: &NativeTaskHandle, next: TaskState) -> ContractResult<()> {
        if self.queue.is_some() {
            return Err(ContractError::InvalidRequest(
                "queue-backed native tasks are driven by the shared task queue".into(),
            ));
        }
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("l2 task lock poisoned".into()))?;

        let record = tasks.get_mut(&handle.handle_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!(
                "native task not found: {}",
                handle.handle_id
            ))
        })?;

        let allowed = record.state.can_transition_to(next);

        if !allowed {
            return Err(ContractError::InvalidRequest(format!(
                "invalid native task transition: {:?} → {:?}",
                record.state, next
            )));
        }

        record.state = next;
        Ok(())
    }

    pub fn task_count(&self) -> usize {
        self.tasks.lock().map(|t| t.len()).unwrap_or(0)
    }

    /// Returns the task with the submission time.
    pub fn task_info(
        &self,
        handle: &NativeTaskHandle,
    ) -> ContractResult<(TaskContract, u64, TaskState)> {
        let tasks = self
            .tasks
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("l2 task lock poisoned".into()))?;
        let record = tasks.get(&handle.handle_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!(
                "native task not found: {}",
                handle.handle_id
            ))
        })?;
        let state = if let Some(queue) = &self.queue {
            queue
                .get(&handle.handle_id)?
                .ok_or_else(|| {
                    ContractError::ResourceUnavailable("queued native task missing".into())
                })?
                .state
        } else {
            record.state
        };
        Ok((
            record.contract.clone(),
            record.submitted_at_unix_secs,
            state,
        ))
    }
}

impl Default for L2NativeTaskProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl INativeTaskProvider for L2NativeTaskProvider {
    fn submit(&self, contract: &TaskContract) -> ContractResult<NativeTaskHandle> {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let handle_id = format!("native-{}-{}", now_unix_secs(), seq);

        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("l2 task lock poisoned".into()))?;

        if let Some(queue) = &self.queue {
            queue.enqueue(QueuedTask::new(
                handle_id.clone(),
                contract.clone(),
                now_unix_ms(),
            ))?;
        }

        tasks.insert(
            handle_id.clone(),
            NativeTaskRecord {
                contract: contract.clone(),
                state: TaskState::Queued,
                submitted_at_unix_secs: now_unix_secs(),
            },
        );

        tracing::info!(
            component = "l2_native_task_provider",
            event = "native_task_submitted",
            handle_id = %handle_id,
            "native task submitted"
        );

        Ok(NativeTaskHandle { handle_id })
    }

    fn query(&self, handle: &NativeTaskHandle) -> ContractResult<TaskState> {
        let tasks = self
            .tasks
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("l2 task lock poisoned".into()))?;
        let record = tasks.get(&handle.handle_id).ok_or_else(|| {
            ContractError::ResourceUnavailable(format!(
                "native task not found: {}",
                handle.handle_id
            ))
        })?;
        if let Some(queue) = &self.queue {
            return queue
                .get(&handle.handle_id)?
                .map(|task| task.state)
                .ok_or_else(|| {
                    ContractError::ResourceUnavailable(format!(
                        "native task not found: {}",
                        handle.handle_id
                    ))
                });
        }
        Ok(record.state)
    }

    fn cancel(&self, handle: &NativeTaskHandle) -> ContractResult<()> {
        if let Some(queue) = &self.queue {
            let tasks = self
                .tasks
                .lock()
                .map_err(|_| ContractError::AdapterUnavailable("l2 task lock poisoned".into()))?;
            if !tasks.contains_key(&handle.handle_id) {
                return Err(ContractError::ResourceUnavailable(format!(
                    "native task not found: {}",
                    handle.handle_id
                )));
            }
            return queue.cancel(&handle.handle_id);
        }
        // Only Pending / Running can be cancelled
        let current = self.query(handle)?;
        match current {
            TaskState::Queued | TaskState::Running => self.transition(handle, TaskState::Cancelled),
            _ => Err(ContractError::InvalidRequest(format!(
                "cannot cancel task in state {:?}",
                current
            ))),
        }
    }

    fn source_name(&self) -> &'static str {
        "l2.native_task_provider"
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
