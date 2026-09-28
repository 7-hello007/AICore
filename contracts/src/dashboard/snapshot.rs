use crate::schema::SchemaVersion;
use serde::{Deserialize, Serialize};

use crate::capability::entry::CapabilityEntry;
use crate::context::snapshot::ContextSnapshot;
use crate::ipc::protocol::PluginEntryDto;
use crate::policy::resource_policy::ResourcePolicy;
use crate::runtime::{
    daemon_status::{DaemonStatus, TickSummary},
    route::RuntimeRoute,
};

/// Shared Dashboard state contract.
///
/// **Terminal and Web Dashboard must share this structure.**
/// Add new fields here first, then update both renderers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DashboardSnapshot {
    pub schema_version: SchemaVersion,
    pub fetched_at_unix_ms: u64,

    // === Core runtime ===
    pub daemon_status: Option<DaemonStatus>,

    // === Current context and task ===
    pub current_context: Option<ContextSnapshot>,
    pub current_workload: Option<String>,
    pub prediction: Option<PredictionSummary>,

    // === Decision ===
    pub current_policy: Option<ResourcePolicy>,
    pub runtime_route: Option<RuntimeRoute>,

    // === Execution ===
    pub execution_status: Option<ExecutionSummary>,
    pub rollback_status: Option<RollbackSummary>,

    // === Capabilities and components ===
    pub capabilities: Vec<CapabilityEntry>,
    pub tier_states: Vec<TierStateEntry>,
    pub plugin_states: Vec<PluginEntryDto>,

    // === Long-term state ===
    pub retention_status: Option<RetentionSummary>,
    pub ai_runtime_health: Option<AiRuntimeHealthSummary>,

    // === History ===
    pub recent_ticks: Vec<TickSummary>,

    // === Errors ===
    pub errors: Vec<String>,
}

impl DashboardSnapshot {
    /// Current schema version.
    pub const SCHEMA_VERSION: SchemaVersion = SchemaVersion::CURRENT;

    pub fn new(fetched_at_unix_ms: u64) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            fetched_at_unix_ms,
            ..Default::default()
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.daemon_status.is_some() && self.errors.is_empty()
    }

    pub fn total_errors(&self) -> usize {
        self.errors.len()
    }
}

/// Summary of projections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionSummary {
    pub predicted_workload: String,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_ms: Option<u64>,
}

/// Executive summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub policy_id: String,
    pub overall_status: String,
    pub outcomes_count: usize,
    pub duration_ms: u64,
}

/// Revert summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackSummary {
    pub triggered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_policy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_policy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub succeeded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Tier status entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierStateEntry {
    pub id: String,
    pub level: String,
    pub state: String,
    pub capabilities: Vec<String>,
}

/// Retention Summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionSummary {
    pub enabled: bool,
    pub retention_days: u32,
    pub records_removed_last_run: usize,
    pub last_run_unix_secs: u64,
}

/// AI Runtime Health Summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRuntimeHealthSummary {
    pub runtime_name: String,
    pub healthy: bool,
    pub loaded_models: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}
