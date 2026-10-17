use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};

/// Local durability based on JSON files.
///
/// Use empty history/ default profile when the file does not exist.
pub struct JsonFileStore {
    history_path: PathBuf,
    profile_path: PathBuf,
    cache: RwLock<Vec<HistoricalRecord>>,
    profile: RwLock<UserProfile>,
    history_load_error: Option<String>,
    profile_load_error: Option<String>,
}

impl JsonFileStore {
    pub fn new(history_path: impl Into<PathBuf>, profile_path: impl Into<PathBuf>) -> Self {
        let history_path = history_path.into();
        let profile_path = profile_path.into();

        let (records, history_load_error) = match load_records_from(&history_path) {
            Ok(records) => (records, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let (profile, profile_load_error) = match load_profile_from(&profile_path) {
            Ok(profile) => (profile, None),
            Err(error) => (UserProfile::default(), Some(error)),
        };

        Self {
            history_path,
            profile_path,
            cache: RwLock::new(records),
            profile: RwLock::new(profile),
            history_load_error,
            profile_load_error,
        }
    }

    fn check_history_loaded(&self) -> ContractResult<()> {
        if let Some(error) = &self.history_load_error {
            return Err(ContractError::AdapterUnavailable(error.clone()));
        }
        Ok(())
    }

    fn check_profile_loaded(&self) -> ContractResult<()> {
        if let Some(error) = &self.profile_load_error {
            return Err(ContractError::AdapterUnavailable(error.clone()));
        }
        Ok(())
    }

    fn persist_history(&self, records: &[HistoricalRecord]) -> ContractResult<()> {
        if let Some(parent) = self.history_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ContractError::AdapterUnavailable(format!("create history dir: {e}"))
            })?;
        }
        let tmp = self.history_path.with_extension("json.tmp");
        let mut content = String::new();
        for record in records {
            let line = serde_json::to_string(record).map_err(|e| {
                ContractError::AdapterUnavailable(format!("serialize history: {}", e))
            })?;
            content.push_str(&line);
            content.push('\n');
        }
        fs::write(&tmp, content)
            .map_err(|e| ContractError::AdapterUnavailable(format!("write temp history: {}", e)))?;
        fs::rename(&tmp, &self.history_path)
            .map_err(|e| ContractError::AdapterUnavailable(format!("replace history: {}", e)))?;
        Ok(())
    }

    fn persist_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        let json = serde_json::to_string_pretty(profile)
            .map_err(|e| ContractError::AdapterUnavailable(format!("serialize profile: {}", e)))?;
        if let Some(parent) = self.profile_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ContractError::AdapterUnavailable(format!("create profile dir: {}", e))
            })?;
        }
        let tmp = self.profile_path.with_extension("json.tmp");
        fs::write(&tmp, json)
            .map_err(|e| ContractError::AdapterUnavailable(format!("write temp profile: {}", e)))?;
        fs::rename(&tmp, &self.profile_path)
            .map_err(|e| ContractError::AdapterUnavailable(format!("replace profile: {}", e)))?;
        Ok(())
    }
}

fn load_records_from(path: &PathBuf) -> Result<Vec<HistoricalRecord>, String> {
    let s = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read history {}: {e}", path.display())),
    };
    if let Ok(records) = serde_json::from_str::<Vec<HistoricalRecord>>(&s) {
        return Ok(records);
    }
    s.lines()
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str::<HistoricalRecord>(line)
                .map_err(|e| format!("parse history {} line {}: {e}", path.display(), index + 1))
        })
        .collect()
}

fn load_profile_from(path: &PathBuf) -> Result<UserProfile, String> {
    match fs::read_to_string(path) {
        Ok(s) => {
            serde_json::from_str(&s).map_err(|e| format!("parse profile {}: {e}", path.display()))
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UserProfile::default()),
        Err(e) => Err(format!("read profile {}: {e}", path.display())),
    }
}

impl IMemoryStore for JsonFileStore {
    fn try_process_source(
        &self,
        limit: usize,
    ) -> ContractResult<aicore_contracts::traits::memory_store::ProcessMemorySource> {
        self.check_history_loaded()?;
        self.check_profile_loaded()?;
        let profile = self
            .profile
            .try_read()
            .map_err(|_| ContractError::ResourceUnavailable("profile busy".into()))?;
        let records = self
            .cache
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
        self.check_history_loaded()?;
        let mut cache = self
            .cache
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        let mut updated = cache.clone();
        updated.push(record.clone());
        self.persist_history(&updated)?;
        *cache = updated;
        Ok(())
    }

    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>> {
        self.check_history_loaded()?;
        let cache = self
            .cache
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        let start = cache.len().saturating_sub(limit);
        Ok(cache[start..].to_vec())
    }

    fn clear_history(&self) -> ContractResult<()> {
        self.check_history_loaded()?;
        let mut cache = self
            .cache
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        self.persist_history(&[])?;
        cache.clear();
        Ok(())
    }

    fn record_count(&self) -> ContractResult<usize> {
        self.check_history_loaded()?;
        let cache = self
            .cache
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        Ok(cache.len())
    }

    fn load_profile(&self) -> ContractResult<UserProfile> {
        self.check_profile_loaded()?;
        let p = self
            .profile
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("profile lock poisoned".into()))?;
        Ok(p.clone())
    }

    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        self.check_profile_loaded()?;
        let mut p = self
            .profile
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("profile lock poisoned".into()))?;
        self.persist_profile(profile)?;
        *p = profile.clone();
        Ok(())
    }

    fn delete_records_before(&self, cutoff_unix_ms: u64) -> ContractResult<usize> {
        self.check_history_loaded()?;
        let snapshot = {
            let mut cache = self
                .cache
                .write()
                .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
            let before = cache.len();
            let retained: Vec<_> = cache
                .iter()
                .filter(|r| r.timestamp_unix_ms >= cutoff_unix_ms)
                .cloned()
                .collect();
            let removed = before - retained.len();
            if removed > 0 {
                self.persist_history(&retained)?;
                *cache = retained;
            }
            removed
        };
        Ok(snapshot)
    }

    fn schema_version(&self) -> ContractResult<u32> {
        Ok(1)
    }

    fn store_name(&self) -> &'static str {
        "json_file"
    }
}
