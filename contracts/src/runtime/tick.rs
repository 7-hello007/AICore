use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;
use crate::execution::status::ExecutionOverallStatus;
use crate::policy::priority::PolicySource;

use super::mode::RuntimeMode;

/// A complete result of a tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TickOutcome {
    pub tick_id: u64,
    pub trace_id: String,
    pub timestamp_unix_ms: u64,
    pub duration_ms: u64,
    pub success: bool,
    pub mode_after: RuntimeMode,
    pub workload_type: WorkloadType,
    pub policy_id: Option<String>,
    pub policy_source: Option<PolicySource>,
    pub validation_accepted: bool,
    pub execution_status: Option<ExecutionOverallStatus>,
    pub message: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicted_workload: Option<WorkloadType>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_confidence: Option<f32>,

    // === V3.2 Phase 3: Runtime Adaptation ===
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_route_source: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_binding_executed: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_binding_reason: Option<String>,

    // === V3.2 Phase 5: Dashboard data plane ===
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_percent: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_usage_percent: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npu_usage_percent: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_usage_percent: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_context_json: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_policy_json: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_route_json: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_runtime_health_json: Option<String>,
}
