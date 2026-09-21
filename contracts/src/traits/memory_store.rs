use crate::errors::{ContractError, ContractResult};
use crate::memory::{profile::UserProfile, record::HistoricalRecord};

/// 记忆存储抽象。
///
/// 实现可以是：
/// - 内存存储（测试 / 短期）
/// - JSON 文件（V1 兼容）
/// - SQLite（V2 默认）
/// - 远端（仅当用户显式允许）
pub trait IMemoryStore: Send + Sync {
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()>;
    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>>;
    fn clear_history(&self) -> ContractResult<()>;
    fn record_count(&self) -> ContractResult<usize>;

    fn load_profile(&self) -> ContractResult<UserProfile>;
    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()>;

    /// 删除指定时间戳之前的记录，返回删除数量。
    ///
    /// 默认实现返回 `NotSupported`，让旧实现无需改动。
    fn delete_records_before(&self, _cutoff_unix_ms: u64) -> ContractResult<usize> {
        Err(ContractError::NotSupported(
            "delete_records_before not supported".to_string(),
        ))
    }

    /// 按 retention_days 应用保留策略，返回删除数量。
    ///
    /// 默认实现基于 `delete_records_before`。
    /// `retention_days == 0` 表示"不删除"。
    fn apply_retention(&self, retention_days: u32) -> ContractResult<usize> {
        if retention_days == 0 {
            return Ok(0);
        }

        let cutoff_unix_ms = now_unix_ms().saturating_sub(retention_days as u64 * 86_400_000);

        self.delete_records_before(cutoff_unix_ms)
    }

    /// 返回 schema version。默认 0 表示无版本管理。
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
