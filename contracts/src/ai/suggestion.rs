use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;
use crate::policy::resource_policy::ResourcePolicy;

use super::task::AiTaskType;

/// AI recommended source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSource {
    Mock,
    LocalModel,
    SmallLanguageModel,
    CloudModel,
}

/// The content of the AI proposal. Different types of tasks correspond to payload.
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

/// The output of AI Provider.
///
/// Note: This is only a recommendation, not a final strategy.
/// Any policy sent to an executor must first pass through a validator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSuggestion {
    pub suggestion_id: String,
    pub source: AiSource,
    pub task: AiTaskType,
    /// Confidence, 0.0-1.0.
    pub confidence: f32,
    /// Human readable explanation.
    pub reason: String,
    pub created_at_unix_ms: u64,
    pub payload: AiSuggestionPayload,
}
