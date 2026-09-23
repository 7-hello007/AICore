use serde::{Deserialize, Serialize};

use crate::{application::id::ApplicationId, context::workload::WorkloadType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentDeployment {
    Local,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentRequest {
    pub application_id: ApplicationId,
    pub text: String,
    pub submitted_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentProposal {
    pub workload_type: WorkloadType,
    pub confidence: f32,
    pub reason: String,
    pub expires_at_unix_ms: u64,
}
