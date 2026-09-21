use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;

use super::{contract::TaskContract, lifecycle::TaskState};

/// 队列中的 task 实体。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueuedTask {
    /// 系统分配的 task_id（若 contract 未提供，队列生成一个）。
    pub task_id: String,
    pub application_id: ApplicationId,
    pub contract: TaskContract,
    pub state: TaskState,
    pub enqueued_at_unix_ms: u64,
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
    /// 完成 / 失败原因。
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

    /// 是否已过期。
    pub fn is_expired(&self, now: u64) -> bool {
        if let Some(deadline) = self.contract.deadline_unix_secs {
            now >= deadline && self.state == TaskState::Queued
        } else {
            false
        }
    }

    /// 有效优先级：deadline 越近越高。
    ///
    /// 返回值越大优先级越高。
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

        // deadline 越近，加分越多；上限 1000。
        //
        // 语义：距 deadline 1 小时内的 task 获得显著加成；
        // 越近加成越大，能盖过 base 优先级差异。
        if let Some(deadline) = self.contract.deadline_unix_secs {
            if deadline > now {
                let time_to_deadline = (deadline - now) as i64;
                let bonus = (3600_i64 - time_to_deadline).clamp(0, 1000);
                base + bonus
            } else {
                // 已过期：负分，下次 dequeue 会被 expire
                base - 1000
            }
        } else {
            base
        }
    }
}
