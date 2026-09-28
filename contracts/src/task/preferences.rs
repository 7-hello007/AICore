use serde::{Deserialize, Serialize};

/// Application of delayed sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencySensitivity {
    Low,
    Medium,
    High,
    Critical,
}

/// Optimal application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerPreference {
    PowerSave,
    Balanced,
    Performance,
}

/// Application of data-processing constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyRequirement {
    /// There is a clear demand for local processing.
    LocalOnly,
    /// Allows cloud handling (user-wide).
    CloudAllowed,
    /// not stated; determined by default policy for AICore.
    Unspecified,
}

/// Hardware demand intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareRequirement {
    NotNeeded,
    Optional,
    Preferred,
    Required,
}
