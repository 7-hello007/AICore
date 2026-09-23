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

/// 内部记录。
struct NativeTaskRecord {
    contract: TaskContract,
    state: TaskState,
    submitted_at_unix_secs: u64,
}

/// L2 原生任务控制提供者。
///
/// 语义：
/// - 应用通过 `submit(contract)` 提交原生任务；返回 handle。
/// - 通过 `query(handle)` 查询状态。
/// - 通过 `cancel(handle)` 请求取消。
/// - 独立模式的 `transition(handle, state)` 由 Core 驱动状态机；绑定共享队列后由队列驱动。
///
/// **约束**：
/// - L2 不直接执行 OS 操作；只管理任务元数据。
/// - 实际执行仍走 Core 的 Validator → Executor 路径。
pub struct L2NativeTaskProvider {
    tasks: Mutex<HashMap<String, NativeTaskRecord>>,
    next_seq: AtomicU64,
    queue: Option<Arc<dyn ITaskQueue>>,
}

impl L2NativeTaskProvider {
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

    /// 状态转换。允许：
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

    /// 返回 task 的 contract 与提交时间。
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
        // 只有 Pending / Running 可取消
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
