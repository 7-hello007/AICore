//! V1 JSON → V2 SQLite migration.
//!
//! Semantics:
//! - Resume matching partial imports; refuse to merge unrelated databases.
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

    let source = JsonFileStore::new(json_history, json_profile);
    // Read and validate the complete source before committing anything. Records,
    // profile and completion marker share one SQLite transaction.
    let records = source.load_records(usize::MAX)?;
    let profile = source.load_profile()?;
    let identity = std::fs::canonicalize(json_history)
        .map_err(|e| ContractError::AdapterUnavailable(e.to_string()))?;
    report = sqlite.import_json_snapshot(&identity.to_string_lossy(), &records, &profile)?;
    if report.skipped {
        return Ok(report);
    }

    // Backup Source File
    let backup_path = json_history.with_extension("json.migrated");
    // Unlike rename, creating a hard link cannot replace an existing archive.
    // If archiving/removal fails, keep the source; the committed marker makes
    // the next import idempotent without overwriting any older backup.
    if let Err(e) = std::fs::hard_link(json_history, backup_path)
        .and_then(|()| std::fs::remove_file(json_history))
    {
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

    migrate_json_to_sqlite(&json_history, &json_profile, &sqlite_path)
        .map_err(|e| ContractError::AdapterUnavailable(format!("migration failed: {}", e)))
}
