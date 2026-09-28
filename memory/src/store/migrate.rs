//! V1 JSON → V2 SQLite migration.
//!
//! Semantics:
//! - If the target SQLite is present and not empty, skip the migration.
//! - If source JSON does not exist, skip.
//! - After successful migration, rename source JSON as `.migrated` suffix and keep backup.

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

/// Move from JSON to SQLite.
///
/// - `json_history`: Source JSON history path.
/// - `json_profile`: Source JSON path.
/// - `sqlite_path`: Target SQLite path.
pub fn migrate_json_to_sqlite(
    json_history: &Path,
    json_profile: &Path,
    sqlite_path: &Path,
) -> ContractResult<MigrationReport> {
    // Signing is & Path, no as_ref()

    let mut report = MigrationReport::default();

    if !json_history.exists() {
        report.skipped = true;
        report.skip_reason = Some(format!("source missing: {}", json_history.display()));
        return Ok(report);
    }

    let sqlite = SqliteMemoryStore::open(sqlite_path)?;

    // Target is not empty, skip
    if sqlite.record_count()? > 0 {
        report.skipped = true;
        report.skip_reason = Some("target sqlite already has records".to_string());
        return Ok(report);
    }

    let source = JsonFileStore::new(json_history, json_profile);

    // Organisation
    let records = source.load_records(1_000_000)?;
    for r in &records {
        sqlite.append_record(r)?;
    }
    report.records_migrated = records.len();

    // Move profile
    let profile = source.load_profile()?;
    if profile.updated_at_unix_secs > 0 {
        sqlite.save_profile(&profile)?;
        report.profile_migrated = true;
    }

    // Backup Source File
    let backup_path = json_history.with_extension("json.migrated");
    if let Err(e) = std::fs::rename(json_history, backup_path) {
        // Migration completed, renaming failed as a warning
        tracing::warn!(
            component = "memory_migration",
            event = "backup_rename_failed",
            error = %e,
            "records migrated but source rename failed"
        );
    }

    Ok(report)
}

/// Default JSON history path.
pub fn default_json_history_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("history.json")
}

/// Default JSON profile path.
pub fn default_json_profile_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("profile.json")
}

/// Default SQLite path.
pub fn default_sqlite_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("memory.db")
}

/// Easy: Move using the default path.
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
