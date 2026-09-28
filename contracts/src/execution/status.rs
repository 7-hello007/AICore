use serde::{Deserialize, Serialize};

/// The result state of the individual execution action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Success,
    Skipped,
    NotSupported,
    PermissionDenied,
    Failed,
}

impl ExecutionStatus {
    /// Is it a real failure? SafeMode）。
    ///
    /// - `Failed`/`PermissionDenied`: What a failure.
    /// - `NotSupported`: Lack of capability is not a failure (should be downgraded).
    /// - `Skipped`: Explicit skip, not a failure.
    pub fn is_real_failure(self) -> bool {
        matches!(self, Self::Failed | Self::PermissionDenied)
    }
}

/// Overall implementation results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOverallStatus {
    AllSucceeded,
    PartiallySucceeded,
    /// All outcomes are Skipped/NotSupported, without any practical operation.
    NoOp,
    AllFailed,
}

impl ExecutionOverallStatus {
    pub fn from_outcomes(outcomes: &[super::result::ExecutionOutcome]) -> Self {
        if outcomes.is_empty() {
            return Self::NoOp;
        }

        let total = outcomes.len();
        let success = outcomes
            .iter()
            .filter(|o| o.status == ExecutionStatus::Success)
            .count();
        let real_failures = outcomes
            .iter()
            .filter(|o| o.status.is_real_failure())
            .count();

        if success == total {
            return Self::AllSucceeded;
        }
        if success > 0 {
            // Successful, non-successful (NotSupported/Skipped/Failed)
            return Self::PartiallySucceeded;
        }
        if real_failures > 0 {
            // No success, no failure, no failure.
            return Self::AllFailed;
        }
        // No success, no real failure. NotSupported / Skipped → NoOp
        Self::NoOp
    }

    /// Does it count as "a friendly success to SafeMode"?
    ///
    /// - AllSucceeded / PartiallySucceeded → true (success, replacement failure count)
    /// - NoOp → false (no change in SafeMode status)
    /// - AllFailed → false
    pub fn is_success(self) -> bool {
        matches!(self, Self::AllSucceeded | Self::PartiallySucceeded)
    }

    /// Is it a real failure? SafeMode）。
    pub fn is_failure(self) -> bool {
        matches!(self, Self::AllFailed)
    }
}
