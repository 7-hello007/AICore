use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};

/// 基于 JSON 文件的本地持久化。
///
/// 文件不存在时按空历史 / 默认 profile 处理，不报错。
pub struct JsonFileStore {
    history_path: PathBuf,
    profile_path: PathBuf,
    cache: RwLock<Vec<HistoricalRecord>>,
    profile: RwLock<UserProfile>,
}

impl JsonFileStore {
    pub fn new(history_path: impl Into<PathBuf>, profile_path: impl Into<PathBuf>) -> Self {
        let history_path = history_path.into();
        let profile_path = profile_path.into();

        let records = load_records_from(&history_path);
        let profile = load_profile_from(&profile_path);

        Self {
            history_path,
            profile_path,
            cache: RwLock::new(records),
            profile: RwLock::new(profile),
        }
    }

    fn persist_history(&self, records: &[HistoricalRecord]) -> ContractResult<()> {
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

fn load_records_from(path: &PathBuf) -> Vec<HistoricalRecord> {
    let Ok(s) = fs::read_to_string(path) else {
        return Vec::new();
    };
    if let Ok(records) = serde_json::from_str::<Vec<HistoricalRecord>>(&s) {
        return records;
    }
    s.lines()
        .filter_map(|line| serde_json::from_str::<HistoricalRecord>(line).ok())
        .collect()
}

fn load_profile_from(path: &PathBuf) -> UserProfile {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => UserProfile::default(),
    }
}

impl IMemoryStore for JsonFileStore {
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()> {
        let mut cache = self
            .cache
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        if let Some(parent) = self.history_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let line = serde_json::to_string(record)
            .map_err(|e| ContractError::AdapterUnavailable(format!("serialize record: {}", e)))?;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_path)
            .map_err(|e| ContractError::AdapterUnavailable(format!("open history: {}", e)))?;
        writeln!(file, "{}", line)
            .map_err(|e| ContractError::AdapterUnavailable(format!("append history: {}", e)))?;
        cache.push(record.clone());
        Ok(())
    }

    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>> {
        let cache = self
            .cache
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        let start = cache.len().saturating_sub(limit);
        Ok(cache[start..].to_vec())
    }

    fn clear_history(&self) -> ContractResult<()> {
        let mut cache = self
            .cache
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        self.persist_history(&[])?;
        cache.clear();
        Ok(())
    }

    fn record_count(&self) -> ContractResult<usize> {
        let cache = self
            .cache
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("history lock poisoned".into()))?;
        Ok(cache.len())
    }

    fn load_profile(&self) -> ContractResult<UserProfile> {
        let p = self
            .profile
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("profile lock poisoned".into()))?;
        Ok(p.clone())
    }

    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        let mut p = self
            .profile
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("profile lock poisoned".into()))?;
        self.persist_profile(profile)?;
        *p = profile.clone();
        Ok(())
    }

    fn delete_records_before(&self, cutoff_unix_ms: u64) -> ContractResult<usize> {
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
