use serde::{Deserialize, Serialize};

/// AI can undertake the type of task.
///
/// This is the list of what AI can do for the Provider capability statement and recommended classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskType {
    ClassifyWorkload,
    PredictLoad,
    PredictRuntime,
    PredictEnergy,
    RecommendPolicy,
    UnderstandIntent,
}
