use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;
use crate::context::workload::WorkloadType;
use crate::policy::priority::PolicyPriority;

use super::preferences::{
    HardwareRequirement, LatencySensitivity, PowerPreference, PrivacyRequirement,
};

/// Apply standard interfaces to AICore that state what I want to do.
///
/// Principle:
/// - State requirements without prescribing the hardware implementation.
/// - AICore determines the actual resource strategy.
/// - AICore extrapolates fields from context and default policy when they are not available.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskContract {
    pub application_id: ApplicationId,
    /// Apply a custom task ID for tracking.
    pub task_id: Option<String>,
    /// workload type;None means to let AICore extrapolation.
    pub task: Option<WorkloadType>,
    /// Overall priority preference.
    pub priority: Option<PolicyPriority>,
    pub latency: Option<LatencySensitivity>,
    pub power: Option<PowerPreference>,
    pub privacy: Option<PrivacyRequirement>,
    pub gpu_requirement: Option<HardwareRequirement>,
    pub npu_requirement: Option<HardwareRequirement>,
    /// Deadline, Unix epoch seconds.
    pub deadline_unix_secs: Option<u64>,
}

impl TaskContract {
    /// Minimum contract: declaration of application_id only.
    pub fn minimal(application_id: ApplicationId) -> Self {
        Self {
            application_id,
            task_id: None,
            task: None,
            priority: None,
            latency: None,
            power: None,
            privacy: None,
            gpu_requirement: None,
            npu_requirement: None,
            deadline_unix_secs: None,
        }
    }
}
