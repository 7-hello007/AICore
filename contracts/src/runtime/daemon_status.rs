use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;
use crate::execution::status::ExecutionOverallStatus;
use crate::policy::priority::PolicySource;
use crate::schema::SchemaVersion;

use super::{mode::RuntimeMode, tick::TickOutcome};

/// Daemon exposed state snapshot schema version.
pub const STATE_SCHEMA_VERSION: SchemaVersion = SchemaVersion::CURRENT;

/// Damon's exposed state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonStatus {
    pub version: SchemaVersion,
    pub pid: u32,
    pub started_at_unix_ms: u64,
    pub last_tick_at_unix_ms: u64,
    pub tick_count: u64,
    pub successful_ticks: u64,
    pub failed_ticks: u64,
    pub current_mode: RuntimeMode,
    pub last_tick: Option<TickSummary>,
}

/// Tick. Summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TickSummary {
    pub tick_id: u64,
    pub trace_id: String,
    pub timestamp_unix_ms: u64,
    pub duration_ms: u64,
    pub success: bool,
    pub workload_type: String,
    pub policy_id: Option<String>,
    pub policy_source: Option<PolicySource>,
    pub execution_status: Option<ExecutionOverallStatus>,
    pub message: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicted_workload: Option<WorkloadType>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_confidence: Option<f32>,

    // === V3.2 Phase 3 ===
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_route_source: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_binding_executed: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_binding_reason: Option<String>,

    // === V3.2 Phase 5 ===
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

impl TickSummary {
    pub fn from_outcome(o: &TickOutcome) -> Self {
        Self {
            tick_id: o.tick_id,
            trace_id: o.trace_id.clone(),
            timestamp_unix_ms: o.timestamp_unix_ms,
            duration_ms: o.duration_ms,
            success: o.success,
            workload_type: format!("{:?}", o.workload_type),
            policy_id: o.policy_id.clone(),
            policy_source: o.policy_source,
            execution_status: o.execution_status,
            message: o.message.clone(),
            predicted_workload: o.predicted_workload,
            prediction_confidence: o.prediction_confidence,
            runtime_route_source: o.runtime_route_source.clone(),
            runtime_binding_executed: o.runtime_binding_executed,
            runtime_binding_reason: o.runtime_binding_reason.clone(),
            cpu_usage_percent: o.cpu_usage_percent,
            gpu_usage_percent: o.gpu_usage_percent,
            npu_usage_percent: o.npu_usage_percent,
            memory_usage_percent: o.memory_usage_percent,
            current_context_json: o.current_context_json.clone(),
            current_policy_json: o.current_policy_json.clone(),
            runtime_route_json: o.runtime_route_json.clone(),
            ai_runtime_health_json: o.ai_runtime_health_json.clone(),
        }
    }
}

impl DaemonStatus {
    pub fn new(pid: u32) -> Self {
        let now = now_unix_ms();
        Self {
            version: STATE_SCHEMA_VERSION,
            pid,
            started_at_unix_ms: now,
            last_tick_at_unix_ms: now,
            tick_count: 0,
            successful_ticks: 0,
            failed_ticks: 0,
            current_mode: RuntimeMode::Normal,
            last_tick: None,
        }
    }

    pub fn record_tick(&mut self, outcome: &TickOutcome) {
        self.tick_count += 1;

        let is_noop = matches!(outcome.execution_status, Some(ExecutionOverallStatus::NoOp));
        if !is_noop {
            if outcome.success {
                self.successful_ticks += 1;
            } else {
                self.failed_ticks += 1;
            }
        }

        self.current_mode = outcome.mode_after;
        self.last_tick_at_unix_ms = now_unix_ms();
        self.last_tick = Some(TickSummary::from_outcome(outcome));
    }
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
