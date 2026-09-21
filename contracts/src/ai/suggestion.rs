use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;
use crate::policy::resource_policy::ResourcePolicy;

use super::task::AiTaskType;

/// AI 建议的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSource {
    Mock,
    LocalModel,
    SmallLanguageModel,
    CloudModel,
}

/// AI 建议的内容。不同任务类型对应不同 payload。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiSuggestionPayload {
    Workload(WorkloadType),
    Load(f32),
    Runtime(u64),
    Energy(f32),
    Policy(ResourcePolicy),
    Intent(String),
}

/// AI Provider 的输出。
///
/// 注意：这只是建议，不是最终策略。
/// 任何进入 Executor 的策略都必须经过 Validator。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSuggestion {
    pub suggestion_id: String,
    pub source: AiSource,
    pub task: AiTaskType,
    /// 置信度，0.0 - 1.0。
    pub confidence: f32,
    /// 人类可读的解释。
    pub reason: String,
    pub created_at_unix_ms: u64,
    pub payload: AiSuggestionPayload,
}
