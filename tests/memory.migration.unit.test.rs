//! JSON → SQLite 迁移测试。

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

    // 准备 JSON 源
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

    // SQLite 已写入
    let sqlite = SqliteMemoryStore::open(&sqlite_path).unwrap();
    assert_eq!(sqlite.record_count().unwrap(), 2);
    let profile = sqlite.load_profile().unwrap();
    assert!(profile.history_enabled);
    assert_eq!(profile.retention_days, 14);

    // 源文件被重命名为 .migrated
    assert!(!json_history.exists());
    let backup = json_history.with_extension("json.migrated");
    assert!(backup.exists());

    // 清理
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

    // 先建 JSON
    {
        let store = JsonFileStore::new(&json_history, &json_profile);
        store.append_record(&rec("a")).unwrap();
    }

    // 先建 SQLite 并写入
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

    // JSON 源应该保留（因为跳过了）
    assert!(json_history.exists());

    let _: PathBuf = sqlite_path.clone();
    let _ = std::fs::remove_file(&json_history);
    let _ = std::fs::remove_file(&sqlite_path);
    let _ = std::fs::remove_dir(&tmp);
}
