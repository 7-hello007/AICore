use serde::{Deserialize, Serialize};

/// 计算机当前正在承担的 workload 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadType {
    Development,
    Gaming,
    VideoEditing,
    AiInference,
    Rendering,
    Compilation,
    FileProcessing,
    VideoConference,
    GeneralOffice,
    BackgroundTask,
    #[default]
    Unknown,
}

/// 用户与系统的交互级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InteractionLevel {
    Interactive,
    Passive,
    #[default]
    Idle,
}
