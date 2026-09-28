use crate::errors::{ContractError, ContractResult};
use crate::memory::{profile::UserProfile, record::HistoricalRecord};

/// Memory store abstraction.
///
/// Implementations may use:
/// - In-memory storage (tests or short-lived state)
/// - JSON files (V1 compatibility)
/// - SQLite (V2 default)
/// - Remote storage (only with explicit user consent)
pub trait IMemoryStore: Send + Sync {
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()>;
    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>>;
    fn clear_history(&self) -> ContractResult<()>;
    fn record_count(&self) -> ContractResult<usize>;

    fn load_profile(&self) -> ContractResult<UserProfile>;
    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()>;

    /// Delete records before the specified timestamp and return the deletion count.
    ///
    /// The default returns `NotSupported` to preserve compatibility with older stores.
    fn delete_records_before(&self, _cutoff_unix_ms: u64) -> ContractResult<usize> {
        Err(ContractError::NotSupported(
            "delete_records_before not supported".to_string(),
        ))
    }

    /// Apply a retention policy and return the deletion count.
    ///
    /// Default implementation is based on `delete_records_before`.
    /// `retention_days == 0` means "do not delete".
    fn apply_retention(&self, retention_days: u32) -> ContractResult<usize> {
        if retention_days == 0 {
            return Ok(0);
        }

        let cutoff_unix_ms = now_unix_ms().saturating_sub(retention_days as u64 * 86_400_000);

        self.delete_records_before(cutoff_unix_ms)
    }

    /// Returns the schema version. Default 0 means no version management.
    fn schema_version(&self) -> ContractResult<u32> {
        Ok(0)
    }

    fn store_name(&self) -> &'static str;
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
