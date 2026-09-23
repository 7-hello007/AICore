use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    task::{lifecycle::TaskState, queued::QueuedTask},
    traits::task_queue::{transition, ITaskQueue},
};

/// 内存 Task 队列。
///
/// - 优先级：`effective_priority`（含 deadline）。
/// - 过期：`dequeue` 时检查 deadline，过期置 Expired。
/// - 线程安全：内部 `Mutex<QueueState>`。
pub struct InMemoryTaskQueue {
    state: Mutex<QueueState>,
}

#[derive(Default)]
struct QueueState {
    /// task_id → QueuedTask
    tasks: HashMap<String, QueuedTask>,
    /// 顺序 id 计数器（用于生成 task_id）。
    next_seq: AtomicU64,
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
    fn enqueue(&self, mut task: QueuedTask) -> ContractResult<String> {
        let mut state = self.lock()?;

        // 若 task_id 为空，生成一个
        if task.task_id.trim().is_empty() {
            let seq = state.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
            task.task_id = format!("task-{}-{}", now_unix_secs(), seq);
        }

        let id = task.task_id.clone();
        if state.tasks.contains_key(&id) {
            return Err(ContractError::InvalidRequest(format!(
                "duplicate task_id: {}",
                id
            )));
        }

        state.tasks.insert(id.clone(), task);
        Ok(id)
    }

    fn dequeue(&self) -> ContractResult<Option<QueuedTask>> {
        let mut state = self.lock()?;
        let now_secs = now_unix_secs();
        let now_ms = now_unix_ms();

        // 1. 过期清理：所有 Queued 且过期的 → Expired
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
        }

        // 2. 找最高优先级的 Queued
        let candidate = state
            .tasks
            .values()
            .filter(|t| t.state == TaskState::Queued)
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
    /// 取消所有仍处于 queued/running 的任务。
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
        }
        Ok(count)
    }

    /// 清理所有任务。主要用于测试与显式用户操作。
    pub fn clear(&self) -> ContractResult<()> {
        let mut state = self.lock()?;
        state.tasks.clear();
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
        Ok(())
    }
}

/// 共享包装，便于同时给 daemon 和 AdaptiveLoop 使用。
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
