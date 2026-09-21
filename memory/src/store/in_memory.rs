use std::sync::RwLock;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};

/// 内存存储。
///
/// 用途：单元测试、短期会话。不持久化。
#[derive(Default)]
pub struct InMemoryStore {
    records: RwLock<Vec<HistoricalRecord>>,
    profile: RwLock<UserProfile>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IMemoryStore for InMemoryStore {
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()> {
        if let Ok(mut records) = self.records.write() {
            records.push(record.clone());
        }
        Ok(())
    }

    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>> {
        let records = self.records.read().map(|r| r.clone()).unwrap_or_default();
        let start = records.len().saturating_sub(limit);
        Ok(records[start..].to_vec())
    }

    fn clear_history(&self) -> ContractResult<()> {
        if let Ok(mut records) = self.records.write() {
            records.clear();
        }
        Ok(())
    }

    fn record_count(&self) -> ContractResult<usize> {
        Ok(self.records.read().map(|r| r.len()).unwrap_or(0))
    }

    fn load_profile(&self) -> ContractResult<UserProfile> {
        Ok(self.profile.read().map(|p| p.clone()).unwrap_or_default())
    }

    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        if let Ok(mut p) = self.profile.write() {
            *p = profile.clone();
        }
        Ok(())
    }

    fn delete_records_before(&self, cutoff_unix_ms: u64) -> ContractResult<usize> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ContractError::LockPoisoned("records lock poisoned".to_string()))?;

        let before = records.len();

        records.retain(|r| r.timestamp_unix_ms >= cutoff_unix_ms);

        Ok(before - records.len())
    }

    fn schema_version(&self) -> ContractResult<u32> {
        Ok(0)
    }

    fn store_name(&self) -> &'static str {
        "in_memory"
    }
}
