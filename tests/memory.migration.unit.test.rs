//! JSON → SQLite migration test.

use std::path::PathBuf;

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::{profile::UserProfile, record::HistoricalRecord},
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{migrate_json_to_sqlite, JsonFileStore, SqliteMemoryStore};

fn rec(id: &str) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
        timestamp_unix_ms: 1000,
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Interactive,
        policy_id: "p".to_string(),
        policy_source: PolicySource::RuleBased,
        overall_status: ExecutionOverallStatus::AllSucceeded,
        cpu_usage_percent: 10.0,
        gpu_usage_percent: None,
        npu_usage_percent: None,
        memory_usage_percent: 40.0,
        power_mode: PowerMode::Balanced,
    }
}

struct MigrationFixture(PathBuf);
impl MigrationFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aicore-p1-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn migrate(&self) -> aicore_contracts::errors::ContractResult<aicore_memory::MigrationReport> {
        migrate_json_to_sqlite(
            &self.0.join("history.json"),
            &self.0.join("profile.json"),
            &self.0.join("memory.db"),
        )
    }
}
impl Drop for MigrationFixture {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(&self.0).unwrap().flatten() {
            if entry.path().is_dir() {
                let _ = std::fs::remove_dir(entry.path());
            } else {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[test]
fn p1_interrupted_migration_resumes_missing_records_and_profile() {
    let f = MigrationFixture::new();
    let source = JsonFileStore::new(f.0.join("history.json"), f.0.join("profile.json"));
    source.append_record(&rec("a")).unwrap();
    source.append_record(&rec("b")).unwrap();
    source
        .save_profile(&UserProfile {
            updated_at_unix_secs: 100,
            retention_days: 14,
            ..Default::default()
        })
        .unwrap();
    let target = SqliteMemoryStore::open(f.0.join("memory.db")).unwrap();
    target.append_record(&rec("a")).unwrap();
    let report = f.migrate().unwrap();
    assert!(!report.skipped);
    assert_eq!(report.records_migrated, 1);
    assert_eq!(target.record_count().unwrap(), 2);
    assert_eq!(target.load_profile().unwrap().retention_days, 14);
    assert!(!f.0.join("history.json").exists());
}

#[test]
fn p1_invalid_profile_does_not_leave_committed_records() {
    let f = MigrationFixture::new();
    let source = JsonFileStore::new(f.0.join("history.json"), f.0.join("profile.json"));
    source.append_record(&rec("a")).unwrap();
    std::fs::write(f.0.join("profile.json"), "{invalid").unwrap();
    assert!(f.migrate().is_err());
    let target = SqliteMemoryStore::open(f.0.join("memory.db")).unwrap();
    assert_eq!(
        target.record_count().unwrap(),
        0,
        "failed import must be atomic"
    );
    std::fs::remove_file(f.0.join("profile.json")).unwrap();
    assert_eq!(f.migrate().unwrap().records_migrated, 1);
    assert_eq!(target.record_count().unwrap(), 1);
}

#[test]
fn p1_archive_failure_retry_does_not_duplicate_or_overwrite_new_profile() {
    let f = MigrationFixture::new();
    let source = JsonFileStore::new(f.0.join("history.json"), f.0.join("profile.json"));
    source.append_record(&rec("a")).unwrap();
    source
        .save_profile(&UserProfile {
            updated_at_unix_secs: 100,
            retention_days: 14,
            ..Default::default()
        })
        .unwrap();
    std::fs::create_dir(f.0.join("history.json.migrated")).unwrap();
    assert_eq!(f.migrate().unwrap().records_migrated, 1);
    let target = SqliteMemoryStore::open(f.0.join("memory.db")).unwrap();
    target
        .save_profile(&UserProfile {
            updated_at_unix_secs: 200,
            retention_days: 7,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(f.migrate().unwrap().records_migrated, 0);
    assert_eq!(target.record_count().unwrap(), 1);
    assert_eq!(target.load_profile().unwrap().retention_days, 7);
}

#[test]
fn p1_record_conflict_rolls_back_preceding_inserts_and_allows_retry() {
    let f = MigrationFixture::new();
    let source = JsonFileStore::new(f.0.join("history.json"), f.0.join("profile.json"));
    source.append_record(&rec("new")).unwrap();
    source.append_record(&rec("a")).unwrap();
    let target = SqliteMemoryStore::open(f.0.join("memory.db")).unwrap();
    let mut conflicting = rec("a");
    conflicting.policy_id = "different".into();
    target.append_record(&conflicting).unwrap();
    assert!(f.migrate().is_err());
    assert_eq!(target.load_records(10).unwrap(), vec![conflicting]);
    assert!(f.0.join("history.json").exists());
    target.clear_history().unwrap();
    assert_eq!(f.migrate().unwrap().records_migrated, 2);
    assert_eq!(target.record_count().unwrap(), 2);
}

#[test]
fn p1_migration_never_replaces_an_existing_archive() {
    let f = MigrationFixture::new();
    let source = JsonFileStore::new(f.0.join("history.json"), f.0.join("profile.json"));
    source.append_record(&rec("a")).unwrap();
    let archive = f.0.join("history.json.migrated");
    std::fs::write(&archive, b"older archive").unwrap();
    assert_eq!(f.migrate().unwrap().records_migrated, 1);
    assert_eq!(std::fs::read(&archive).unwrap(), b"older archive");
    assert!(
        f.0.join("history.json").exists(),
        "archive collision must preserve the source"
    );
}

#[test]
fn migrate_missing_source_returns_skipped() {
    let tmp = std::env::temp_dir().join(format!("aicore-mig-missing-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);

    let json_history = tmp.join("nonexistent.json");
    let json_profile = tmp.join("nonexistent-profile.json");
    let sqlite_path = tmp.join("memory.db");
    let _ = std::fs::remove_file(&sqlite_path);

    let report = migrate_json_to_sqlite(
        json_history.as_path(),
        json_profile.as_path(),
        sqlite_path.as_path(),
    )
    .unwrap();
    assert!(report.skipped);
    assert_eq!(report.records_migrated, 0);

    let _ = std::fs::remove_dir(&tmp);
}

#[test]
fn migrate_transfers_records_and_profile() {
    let tmp = std::env::temp_dir().join(format!("aicore-mig-full-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let json_history = tmp.join("history.json");
    let json_profile = tmp.join("profile.json");
    let sqlite_path = tmp.join("memory.db");

    let _ = std::fs::remove_file(&json_history);
    let _ = std::fs::remove_file(&json_profile);
    let _ = std::fs::remove_file(&sqlite_path);

    // Prepare JSON source
    {
        let store = JsonFileStore::new(&json_history, &json_profile);
        store.append_record(&rec("a")).unwrap();
        store.append_record(&rec("b")).unwrap();
        let profile = UserProfile {
            history_enabled: true,
            retention_days: 14,
            updated_at_unix_secs: 1_700_000_000,
            ..Default::default()
        };
        store.save_profile(&profile).unwrap();
    }

    let report = migrate_json_to_sqlite(
        json_history.as_path(),
        json_profile.as_path(),
        sqlite_path.as_path(),
    )
    .unwrap();
    assert!(!report.skipped, "got skip: {:?}", report.skip_reason);
    assert_eq!(report.records_migrated, 2);
    assert!(report.profile_migrated);

    // SQLite written
    let sqlite = SqliteMemoryStore::open(&sqlite_path).unwrap();
    assert_eq!(sqlite.record_count().unwrap(), 2);
    let profile = sqlite.load_profile().unwrap();
    assert!(profile.history_enabled);
    assert_eq!(profile.retention_days, 14);

    // Source file renamed.
    assert!(!json_history.exists());
    let backup = json_history.with_extension("json.migrated");
    assert!(backup.exists());

    // Clear
    let _ = std::fs::remove_file(&sqlite_path);
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::remove_file(&json_profile);
    let _ = std::fs::remove_dir(&tmp);
}

#[test]
fn migrate_skips_when_target_non_empty() {
    let tmp = std::env::temp_dir().join(format!("aicore-mig-nonempty-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let json_history = tmp.join("history.json");
    let json_profile = tmp.join("profile.json");
    let sqlite_path = tmp.join("memory.db");

    let _ = std::fs::remove_file(&json_history);
    let _ = std::fs::remove_file(&json_profile);
    let _ = std::fs::remove_file(&sqlite_path);

    // Build JSON first
    {
        let store = JsonFileStore::new(&json_history, &json_profile);
        store.append_record(&rec("a")).unwrap();
    }

    // Build SQLite and write first
    {
        let sqlite = SqliteMemoryStore::open(&sqlite_path).unwrap();
        sqlite.append_record(&rec("existing")).unwrap();
    }

    let report = migrate_json_to_sqlite(
        json_history.as_path(),
        json_profile.as_path(),
        sqlite_path.as_path(),
    )
    .unwrap();
    assert!(report.skipped);
    assert!(report.skip_reason.as_ref().unwrap().contains("already"));

    // JSON source should be retained (because of skipping)
    assert!(json_history.exists());

    let _: PathBuf = sqlite_path.clone();
    let _ = std::fs::remove_file(&json_history);
    let _ = std::fs::remove_file(&sqlite_path);
    let _ = std::fs::remove_dir(&tmp);
}
