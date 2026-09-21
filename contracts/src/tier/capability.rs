use serde::{Deserialize, Serialize};

/// Tier 可提供的能力。
///
/// Core 只通过能力发现工作；不允许用 `match tier.id`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierCapability {
    /// 观察 workload（进程 / 窗口 / 资源信号）。
    WorkloadObservation,
    /// 生成 / 提供 TaskMetadata。
    TaskMetadata,
    /// 生成 / 声明 ResourceHints。
    ResourceHints,
    /// 提供原生任务控制（仅 L2）。
    NativeTaskControl,
    /// 提供用户意图。
    UserIntent,
    /// 透明适配（Observe → Decide → Apply）。
    TransparentAdaptation,
}

impl TierCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkloadObservation => "workload_observation",
            Self::TaskMetadata => "task_metadata",
            Self::ResourceHints => "resource_hints",
            Self::NativeTaskControl => "native_task_control",
            Self::UserIntent => "user_intent",
            Self::TransparentAdaptation => "transparent_adaptation",
        }
    }
}
