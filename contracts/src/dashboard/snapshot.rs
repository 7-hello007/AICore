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

/// Dashboard 共享状态契约。
///
/// **Terminal 与 Web Dashboard 必须共用此结构**。
/// 新增任何字段都先扩展本契约，再同时增加两种渲染。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DashboardSnapshot {
    pub schema_version: SchemaVersion,
    pub fetched_at_unix_ms: u64,

    // === 核心运行时 ===
    pub daemon_status: Option<DaemonStatus>,

    // === 当前上下文与任务 ===
    pub current_context: Option<ContextSnapshot>,
    pub current_workload: Option<String>,
    pub prediction: Option<PredictionSummary>,

    // === 决策 ===
    pub current_policy: Option<ResourcePolicy>,
    pub runtime_route: Option<RuntimeRoute>,

    // === 执行 ===
    pub execution_status: Option<ExecutionSummary>,
    pub rollback_status: Option<RollbackSummary>,

    // === 能力与组件 ===
    pub capabilities: Vec<CapabilityEntry>,
    pub tier_states: Vec<TierStateEntry>,
    pub plugin_states: Vec<PluginEntryDto>,

    // === 长期状态 ===
    pub retention_status: Option<RetentionSummary>,
    pub ai_runtime_health: Option<AiRuntimeHealthSummary>,

    // === 历史 ===
    pub recent_ticks: Vec<TickSummary>,

    // === 错误 ===
    pub errors: Vec<String>,
}

impl DashboardSnapshot {
    /// 当前 schema 版本。
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

/// 预测摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionSummary {
    pub predicted_workload: String,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_ms: Option<u64>,
}

/// 执行摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub policy_id: String,
    pub overall_status: String,
    pub outcomes_count: usize,
    pub duration_ms: u64,
}

/// 回退摘要。
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

/// Tier 状态条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierStateEntry {
    pub id: String,
    pub level: String,
    pub state: String,
    pub capabilities: Vec<String>,
}

/// Retention 摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionSummary {
    pub enabled: bool,
    pub retention_days: u32,
    pub records_removed_last_run: usize,
    pub last_run_unix_secs: u64,
}

/// AI Runtime 健康摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRuntimeHealthSummary {
    pub runtime_name: String,
    pub healthy: bool,
    pub loaded_models: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}
