use serde::{Deserialize, Serialize};

/// AI 可以承担的任务类型。
///
/// 这是"AI 能做什么"的枚举，用于 Provider 能力声明与建议分类。
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
