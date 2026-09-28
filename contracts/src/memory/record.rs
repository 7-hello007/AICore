use crate::application::id::ApplicationId;
use serde::{Deserialize, Serialize};

use crate::context::workload::{InteractionLevel, WorkloadType};
use crate::execution::status::ExecutionOverallStatus;
use crate::policy::priority::PolicySource;
use crate::policy::resource_policy::PowerMode;

/// A historical implementation record.
///
/// Record "sometimes, some workload, produce a policy, execute something."
/// For personalization, forecasting, audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoricalRecord {
    pub record_id: String,
    /// Recorded only for an explicitly registered task, under history consent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
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
