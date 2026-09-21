use serde::{Deserialize, Serialize};

use super::priority::{PolicyPriority, PolicySource};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CpuPolicy {
    pub priority: PolicyPriority,
    /// 可选的 CPU 亲和性；None 表示不限制。
    pub affinity_cores: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GpuPolicy {
    pub priority: PolicyPriority,
    /// 可选的 workload 偏好："graphics" / "compute" / "auto"。
    pub workload_preference: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NpuPolicy {
    pub priority: PolicyPriority,
    /// 是否建议优先使用 NPU 承担推理。
    pub preferred: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryPolicy {
    pub priority: PolicyPriority,
    /// 可选的内存使用上限百分比，0.0 - 100.0。
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
    /// 是否抑制后台任务。
    pub throttle: bool,
    /// 抑制时，后台任务 CPU 使用上限百分比。
    pub max_cpu_percent: Option<f32>,
}

/// AICore 的核心策略对象。
///
/// 生成后必须经过 IPolicyValidator 才能进入 Executor。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourcePolicy {
    pub policy_id: String,
    pub task_id: Option<String>,
    /// 整体优先级；约定 >= 所有子资源优先级。
    pub priority: PolicyPriority,
    pub source: PolicySource,
    /// 人类可读的可解释原因。
    pub reason: String,
    pub cpu: CpuPolicy,
    pub gpu: GpuPolicy,
    pub npu: NpuPolicy,
    pub memory: MemoryPolicy,
    pub power: PowerPolicy,
    pub background: BackgroundPolicy,
    /// 生成时间，Unix epoch 秒。
    pub created_at_unix_ms: u64,
}
