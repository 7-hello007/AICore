use serde::{Deserialize, Serialize};

use crate::policy::resource_policy::ResourcePolicy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyUpdated {
    pub version: String,
    pub timestamp_unix_ms: u64,
    pub policy: ResourcePolicy,
}
