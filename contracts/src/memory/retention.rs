use serde::{Deserialize, Serialize};

/// Keeps the policy configuration.
///
/// Semantics:
/// `enabled = false`: Not self-execution (prior privacy default).
/// - `retention_days`** Not here**: provided by `UserProfile.retention_days`
///   To avoid two configuration conflicts.
/// - `check_interval_secs`: interval between minimum two run times.
/// - `audit_capacity`: Audit of the capacity of the ring buffer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub enabled: bool,
    pub check_interval_secs: u64,
    pub audit_capacity: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            check_interval_secs: 3600,
            audit_capacity: 100,
        }
    }
}

impl RetentionPolicy {
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.check_interval_secs == 0 {
            return Err(AicoreError::ContractInvalid(
                "check_interval_secs must be > 0".to_string(),
            ));
        }
        if self.audit_capacity == 0 {
            return Err(AicoreError::ContractInvalid(
                "audit_capacity must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// The result of a retentation running at a time (also as an audit entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionReport {
    pub ran_at_unix_ms: u64,
    pub retention_days: u32,
    pub records_removed: usize,
    pub success: bool,
    pub skipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RetentionReport {
    pub fn skipped(now: u64, retention_days: u32, reason: impl Into<String>) -> Self {
        Self {
            ran_at_unix_ms: now,
            retention_days,
            records_removed: 0,
            success: false,
            skipped: true,
            message: Some(reason.into()),
        }
    }

    pub fn success(now: u64, retention_days: u32, removed: usize) -> Self {
        Self {
            ran_at_unix_ms: now,
            retention_days,
            records_removed: removed,
            success: true,
            skipped: false,
            message: None,
        }
    }

    pub fn failure(now: u64, retention_days: u32, message: impl Into<String>) -> Self {
        Self {
            ran_at_unix_ms: now,
            retention_days,
            records_removed: 0,
            success: false,
            skipped: false,
            message: Some(message.into()),
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn did_run(&self) -> bool {
        !self.skipped
    }
}
