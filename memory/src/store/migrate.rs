//! V1 JSON → V2 SQLite 迁移。
//!
//! 语义：
//! - 若目标 SQLite 已存在且非空，跳过迁移。
//! - 若源 JSON 不存在，跳过。
//! - 迁移成功后，将源 JSON 重命名为 `.migrated` 后缀，保留备份。

use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::memory_store::IMemoryStore,
};

use super::{json_file::JsonFileStore, sqlite::SqliteMemoryStore};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MigrationReport {
    pub records_migrated: usize,
    pub profile_migrated: bool,
    pub skipped: bool,
    pub skip_reason: Option<String>,
}

/// 从 JSON 迁移到 SQLite。
///
/// - `json_history`：源 JSON history 路径。
/// - `json_profile`：源 JSON profile 路径。
/// - `sqlite_path`：目标 SQLite 路径。
pub fn migrate_json_to_sqlite(
    json_history: &Path,
    json_profile: &Path,
    sqlite_path: &Path,
) -> ContractResult<MigrationReport> {
    // 签名已是 &Path，无需再 as_ref()

    let mut report = MigrationReport::default();

    if !json_history.exists() {
        report.skipped = true;
        report.skip_reason = Some(format!("source missing: {}", json_history.display()));
        return Ok(report);
    }

    let sqlite = SqliteMemoryStore::open(sqlite_path)?;

    // 目标非空则跳过
    if sqlite.record_count()? > 0 {
        report.skipped = true;
        report.skip_reason = Some("target sqlite already has records".to_string());
        return Ok(report);
    }

    let source = JsonFileStore::new(json_history, json_profile);

    // 迁移 records
    let records = source.load_records(1_000_000)?;
    for r in &records {
        sqlite.append_record(r)?;
    }
    report.records_migrated = records.len();

    // 迁移 profile
    let profile = source.load_profile()?;
    if profile.updated_at_unix_secs > 0 {
        sqlite.save_profile(&profile)?;
        report.profile_migrated = true;
    }

    // 备份源文件
    let backup_path = json_history.with_extension("json.migrated");
    if let Err(e) = std::fs::rename(json_history, backup_path) {
        // 迁移已完成，重命名失败只是警告
        tracing::warn!(
            component = "memory_migration",
            event = "backup_rename_failed",
            error = %e,
            "records migrated but source rename failed"
        );
    }

    Ok(report)
}

/// 默认 JSON history 路径。
pub fn default_json_history_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("history.json")
}

/// 默认 JSON profile 路径。
pub fn default_json_profile_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("profile.json")
}

/// 默认 SQLite 路径。
pub fn default_sqlite_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("memory.db")
}

/// 便捷：使用默认路径迁移。
pub fn migrate_with_defaults() -> ContractResult<MigrationReport> {
    let json_history = default_json_history_path();
    let json_profile = default_json_profile_path();
    let sqlite_path = default_sqlite_path();

    if !json_history.exists() {
        return Ok(MigrationReport {
            skipped: true,
            skip_reason: Some("no legacy json history".to_string()),
            ..Default::default()
        });
    }

    if sqlite_path.exists()
        && SqliteMemoryStore::open(&sqlite_path)
            .and_then(|s| s.record_count())
            .unwrap_or(0)
            > 0
    {
        return Ok(MigrationReport {
            skipped: true,
            skip_reason: Some("sqlite already initialized".to_string()),
            ..Default::default()
        });
    }

    migrate_json_to_sqlite(&json_history, &json_profile, &sqlite_path)
        .map_err(|e| ContractError::AdapterUnavailable(format!("migration failed: {}", e)))
}
