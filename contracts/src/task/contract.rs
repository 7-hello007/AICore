use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;
use crate::context::workload::WorkloadType;
use crate::policy::priority::PolicyPriority;

use super::preferences::{
    HardwareRequirement, LatencySensitivity, PowerPreference, PrivacyRequirement,
};

/// 应用向 AICore 声明"我要完成什么"的标准接口。
///
/// 原则：
/// - 应用只声明需求，不指定具体硬件实现。
/// - AICore 决定实际资源策略。
/// - 未提供字段时由 AICore 根据上下文和默认策略推断。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskContract {
    pub application_id: ApplicationId,
    /// 应用自定义的任务 ID，用于追踪。
    pub task_id: Option<String>,
    /// workload 类型；None 表示让 AICore 推断。
    pub task: Option<WorkloadType>,
    /// 整体优先级偏好。
    pub priority: Option<PolicyPriority>,
    pub latency: Option<LatencySensitivity>,
    pub power: Option<PowerPreference>,
    pub privacy: Option<PrivacyRequirement>,
    pub gpu_requirement: Option<HardwareRequirement>,
    pub npu_requirement: Option<HardwareRequirement>,
    /// 截止时间，Unix epoch 秒。
    pub deadline_unix_secs: Option<u64>,
}

impl TaskContract {
    /// 最小契约：只声明 application_id。
    pub fn minimal(application_id: ApplicationId) -> Self {
        Self {
            application_id,
            task_id: None,
            task: None,
            priority: None,
            latency: None,
            power: None,
            privacy: None,
            gpu_requirement: None,
            npu_requirement: None,
            deadline_unix_secs: None,
        }
    }
}
