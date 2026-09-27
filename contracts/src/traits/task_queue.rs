use crate::errors::ContractResult;
use crate::execution::targets::ExecutionTargets;
use crate::task::{lifecycle::TaskState, queued::QueuedTask};

/// Rechecked immediately before preparation actuation. Implemented by the
/// application boundary; queue and Core never resolve OS identity themselves.
pub trait IPreparationAuthorization: Send + Sync {
    fn targets(&self) -> ContractResult<ExecutionTargets>;
}

/// Task 队列抽象。
///
/// 约束：
/// - **不直接访问 OS**。
/// - 线程安全。
/// - `dequeue` 是原子操作；并发调用不会返回同一 task。
pub trait ITaskQueue: Send + Sync {
    /// 入队。返回系统分配的 task_id。
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

    /// 取下一个最高优先级的 Queued task，原子地将状态置为 Running。
    ///
    /// - 返回 `None` 表示队列为空。
    /// - 已过期的 task 会被自动标记为 Expired，不返回。
    fn dequeue(&self) -> ContractResult<Option<QueuedTask>>;

    /// 批量取出最高优先级任务。默认实现逐个调用 `dequeue`，避免各实现重复维护批处理语义。
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

    /// 标记 task 完成。
    fn complete(&self, task_id: &str, message: Option<String>) -> ContractResult<()>;

    /// 标记 task 失败。
    fn fail(&self, task_id: &str, message: String) -> ContractResult<()>;

    /// 取消 task。
    fn cancel(&self, task_id: &str) -> ContractResult<()>;

    /// 查询 task 状态。
    fn get(&self, task_id: &str) -> ContractResult<Option<QueuedTask>>;

    /// 返回所有 task 的快照（包括终态）。
    fn snapshot(&self) -> ContractResult<Vec<QueuedTask>>;

    /// 队列中 Queued 状态的 task 数。
    fn queued_count(&self) -> ContractResult<usize>;

    /// 队列名。
    fn queue_name(&self) -> &'static str;
}

/// 根据当前状态推导的下一个状态（辅助函数）。
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
