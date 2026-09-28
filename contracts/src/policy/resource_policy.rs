use serde::{Deserialize, Serialize};

use super::priority::{PolicyPriority, PolicySource};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CpuPolicy {
    pub priority: PolicyPriority,
    /// Optional CPU homogeneity; None indicates unlimited.
    pub affinity_cores: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GpuPolicy {
    pub priority: PolicyPriority,
    /// Optional workload preferences: "graphics"/ "compute"/ "auto".
    pub workload_preference: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NpuPolicy {
    pub priority: PolicyPriority,
    /// Whether to prefer the NPU for inference.
    pub preferred: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryPolicy {
    pub priority: PolicyPriority,
    /// Optional memory use ceiling percentage, 0.0 - 100.0.
    pub max_usage_percent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PowerMode {
    Performance,
    #[default]
    Balanced,
    PowerSave,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PowerPolicy {
    pub mode: PowerMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BackgroundPolicy {
    /// Whether to throttle background tasks.
    pub throttle: bool,
    /// When suppressed, CPU is used as the maximum percentage.
    pub max_cpu_percent: Option<f32>,
}

/// Core policy object for AICore.
///
/// After generation, the policy must pass `IPolicyValidator` before reaching an executor.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourcePolicy {
    pub policy_id: String,
    pub task_id: Option<String>,
    /// Overall priority; agreed > = all sub-resource priorities.
    pub priority: PolicyPriority,
    pub source: PolicySource,
    /// Human readable explanations.
    pub reason: String,
    pub cpu: CpuPolicy,
    pub gpu: GpuPolicy,
    pub npu: NpuPolicy,
    pub memory: MemoryPolicy,
    pub power: PowerPolicy,
    pub background: BackgroundPolicy,
    /// Generating time, Unix epoch millisecond.
    pub created_at_unix_ms: u64,
}
