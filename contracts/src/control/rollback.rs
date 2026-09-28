use serde::{Deserialize, Serialize};

/// Turn back the record.
///
/// Each control attempt failed and was generated back; used for audit and tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackRecord {
    /// Whether or not to trigger a retreat.
    pub triggered: bool,
    /// (None indicates no previous article).
    pub from_policy_id: Option<String>,
    /// Backward target id (None means no available return target).
    pub to_policy_id: Option<String>,
    /// Reason for retreat.
    pub reason: Option<String>,
    /// Whether or not the retreat was successful.
    pub succeeded: Option<bool>,
}

impl RollbackRecord {
    pub fn not_triggered() -> Self {
        Self {
            triggered: false,
            from_policy_id: None,
            to_policy_id: None,
            reason: None,
            succeeded: None,
        }
    }

    pub fn triggered(
        from: Option<String>,
        to: Option<String>,
        reason: impl Into<String>,
        succeeded: bool,
    ) -> Self {
        Self {
            triggered: true,
            from_policy_id: from,
            to_policy_id: to,
            reason: Some(reason.into()),
            succeeded: Some(succeeded),
        }
    }
}

impl Default for RollbackRecord {
    fn default() -> Self {
        Self::not_triggered()
    }
}
