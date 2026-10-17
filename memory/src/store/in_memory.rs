use std::sync::RwLock;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};

/// Memory storage.
///
/// Purpose: Module testing, short-term sessions. Not durable.
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
    fn try_process_source(
        &self,
        limit: usize,
    ) -> ContractResult<aicore_contracts::traits::memory_store::ProcessMemorySource> {
        let profile = self
            .profile
            .try_read()
            .map_err(|_| ContractError::ResourceUnavailable("profile busy".into()))?;
        let records = self
            .records
            .try_read()
            .map_err(|_| ContractError::ResourceUnavailable("history busy".into()))?;
        Ok(
            aicore_contracts::traits::memory_store::ProcessMemorySource::Snapshot {
                profile: profile.clone(),
                records: records[records.len().saturating_sub(limit)..].to_vec(),
            },
        )
    }
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ContractError::LockPoisoned("records lock poisoned".to_string()))?;
        records.push(record.clone());
        Ok(())
    }

    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>> {
        let records = self
            .records
            .read()
            .map_err(|_| ContractError::LockPoisoned("records lock poisoned".to_string()))?
            .clone();
        let start = records.len().saturating_sub(limit);
        Ok(records[start..].to_vec())
    }

    fn clear_history(&self) -> ContractResult<()> {
        let mut records = self
            .records
            .write()
            .map_err(|_| ContractError::LockPoisoned("records lock poisoned".to_string()))?;
        records.clear();
        Ok(())
    }

    fn record_count(&self) -> ContractResult<usize> {
        Ok(self
            .records
            .read()
            .map_err(|_| ContractError::LockPoisoned("records lock poisoned".to_string()))?
            .len())
    }

    fn load_profile(&self) -> ContractResult<UserProfile> {
        Ok(self
            .profile
            .read()
            .map_err(|_| ContractError::LockPoisoned("profile lock poisoned".to_string()))?
            .clone())
    }

    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        let mut p = self
            .profile
            .write()
            .map_err(|_| ContractError::LockPoisoned("profile lock poisoned".to_string()))?;
        *p = profile.clone();
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
