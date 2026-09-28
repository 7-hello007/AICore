use serde::{Deserialize, Serialize};

/// Capabilities offered by a tier.
///
/// Core discovers work only through capabilities; `match tier.id` is not allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierCapability {
    /// Watch the workload (process/ window/ resource signal).
    WorkloadObservation,
    /// Generates/ provides TaskMetadata.
    TaskMetadata,
    /// Generate/ declare ResourceHints.
    ResourceHints,
    /// Provide primary mission control (L2 only).
    NativeTaskControl,
    /// Provide user intent.
    UserIntent,
    /// Transparency fit (Observe → Decide → Apply).
    TransparentAdaptation,
}

impl TierCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkloadObservation => "workload_observation",
            Self::TaskMetadata => "task_metadata",
            Self::ResourceHints => "resource_hints",
            Self::NativeTaskControl => "native_task_control",
            Self::UserIntent => "user_intent",
            Self::TransparentAdaptation => "transparent_adaptation",
        }
    }
}
