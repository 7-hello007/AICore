use serde::{Deserialize, Serialize};

/// The computer is currently assuming the workload type.
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

/// Level of user-system interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InteractionLevel {
    Interactive,
    Passive,
    #[default]
    Idle,
}
