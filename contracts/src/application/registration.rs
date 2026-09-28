use serde::{Deserialize, Serialize};

use super::id::ApplicationId;

/// Apply registration information.
///
/// Once registered in AICore, applications are eligible to submit TaskContract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationRegistration {
    pub application_id: ApplicationId,
    pub name: String,
    pub version: String,
    pub registered_at_unix_secs: u64,
    /// Application of asserted capabilities, for example, ["gpu.compute", "npu.reference".
    /// AICore allows for more precise matching of capabilities.
    pub capabilities: Vec<String>,
}
