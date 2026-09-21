use serde::{Deserialize, Serialize};

use crate::context::workload::{InteractionLevel, WorkloadType};
use crate::execution::status::ExecutionOverallStatus;
use crate::policy::priority::PolicySource;
use crate::policy::resource_policy::PowerMode;

/// 一条历史执行记录。
///
/// 记录"某时刻、某 workload、产生某 policy、执行得到某结果"。
/// 用于个人化、预测、审计。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoricalRecord {
    pub record_id: String,
    pub timestamp_unix_ms: u64,
    pub workload_type: WorkloadType,
    pub interaction_level: InteractionLevel,
    pub policy_id: String,
    pub policy_source: PolicySource,
    pub overall_status: ExecutionOverallStatus,
    pub cpu_usage_percent: f32,
    pub gpu_usage_percent: Option<f32>,
    pub npu_usage_percent: Option<f32>,
    pub memory_usage_percent: f32,
    pub power_mode: PowerMode,
}
