use serde::{Deserialize, Serialize};

use crate::ai::suggestion::AiSuggestion;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestionCreated {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub suggestion: AiSuggestion,
}
