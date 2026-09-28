use serde::{Deserialize, Serialize};

use crate::context::ContextSnapshot;

/// Releases when Context Engineering creates a new context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUpdated {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub context: ContextSnapshot,
}
