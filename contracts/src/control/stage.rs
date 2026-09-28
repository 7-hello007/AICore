use serde::{Deserialize, Serialize};

/// Six phases of transparent control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStage {
    Observe,
    Decide,
    Validate,
    Apply,
    Verify,
    Rollback,
}

impl ControlStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Decide => "decide",
            Self::Validate => "validate",
            Self::Apply => "apply",
            Self::Verify => "verify",
            Self::Rollback => "rollback",
        }
    }
}

/// Status of implementation of individual phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Success,
    Skipped,
    Failed,
    TimedOut,
}

impl StageStatus {
    pub fn is_success(self) -> bool {
        matches!(self, Self::Success | Self::Skipped)
    }
}

/// Implementation results for individual phases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageOutcome {
    pub stage: ControlStage,
    pub status: StageStatus,
    pub detail: String,
    pub duration_ms: u64,
}

impl StageOutcome {
    pub fn success(stage: ControlStage, duration_ms: u64, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: StageStatus::Success,
            detail: detail.into(),
            duration_ms,
        }
    }

    pub fn skipped(stage: ControlStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: StageStatus::Skipped,
            detail: detail.into(),
            duration_ms: 0,
        }
    }

    pub fn failed(stage: ControlStage, duration_ms: u64, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: StageStatus::Failed,
            detail: detail.into(),
            duration_ms,
        }
    }

    pub fn timed_out(stage: ControlStage, duration_ms: u64, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: StageStatus::TimedOut,
            detail: detail.into(),
            duration_ms,
        }
    }
}
