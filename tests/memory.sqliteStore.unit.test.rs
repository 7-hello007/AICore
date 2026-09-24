//! SqliteMemoryStore 单元测试。

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::record::HistoricalRecord,
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{SqliteMemoryStore, CURRENT_SCHEMA_VERSION};

const BASE_MS: u64 = 1_700_000_000_000;

fn rec(id: &str, ts: u64, w: WorkloadType) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
        timestamp_unix_ms: ts,
        workload_type: w,
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
fn open_in_memory_sets_schema_version() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    assert_eq!(store.store_name(), "sqlite");
}

#[test]
fn append_and_count() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    assert_eq!(store.record_count().unwrap(), 0);

    store
        .append_record(&rec("a", BASE_MS, WorkloadType::Development))
        .unwrap();

    store
        .append_record(&rec("b", BASE_MS + 1_000, WorkloadType::Gaming))
        .unwrap();

    assert_eq!(store.record_count().unwrap(), 2);
}

#[test]
fn load_records_respects_limit() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    for i in 0..5 {
        store
            .append_record(&rec(
                &format!("r{}", i),
                BASE_MS + i as u64,
                WorkloadType::Development,
            ))
            .unwrap();
    }

    let loaded = store.load_records(3).unwrap();

    assert_eq!(loaded.len(), 3);

    // 应返回"最近 3 条"，按时间升序（旧 → 新），
    // 与 InMemoryStore / JsonFileStore 的语义一致。
    assert_eq!(loaded[0].record_id, "r2");
    assert_eq!(loaded[1].record_id, "r3");
    assert_eq!(loaded[2].record_id, "r4");
}

#[test]
fn delete_records_before_cutoff() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    let old_ts = BASE_MS;
    let new_ts = BASE_MS + 1_000;
    let cutoff = BASE_MS + 500;

    store
        .append_record(&rec("old", old_ts, WorkloadType::Development))
        .unwrap();

    store
        .append_record(&rec("new", new_ts, WorkloadType::Development))
        .unwrap();

    let removed = store.delete_records_before(cutoff).unwrap();

    assert_eq!(removed, 1);
    assert_eq!(store.record_count().unwrap(), 1);
}

#[test]
fn retention_uses_profile_retention_days() {
    use aicore_contracts::memory::profile::UserProfile;

    let store = SqliteMemoryStore::open_in_memory().unwrap();

    // 一条很老的记录。
    store
        .append_record(&rec("ancient", 1, WorkloadType::Development))
        .unwrap();

    // 一条明显处于未来的记录，用于确保不会被 retention 删除。
    store
        .append_record(&rec("recent", u64::MAX / 2, WorkloadType::Gaming))
        .unwrap();

    let profile = UserProfile {
        retention_days: 7,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    let removed = store.apply_retention(profile.retention_days).unwrap();

    assert_eq!(removed, 1);
    assert_eq!(store.record_count().unwrap(), 1);
}

#[test]
fn profile_roundtrip() {
    use aicore_contracts::memory::profile::UserProfile;

    let store = SqliteMemoryStore::open_in_memory().unwrap();

    let profile = UserProfile {
        history_enabled: true,
        retention_days: 30,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    let loaded = store.load_profile().unwrap();

    assert!(loaded.history_enabled);
    assert_eq!(loaded.retention_days, 30);
}

#[test]
fn clear_history_removes_all() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    store
        .append_record(&rec("a", BASE_MS, WorkloadType::Development))
        .unwrap();

    store.clear_history().unwrap();

    assert_eq!(store.record_count().unwrap(), 0);
}

#[test]
fn persistence_across_reopen() {
    let tmp = std::env::temp_dir().join(format!("aicore-sqlite-test-{}", std::process::id()));

    let _ = std::fs::create_dir_all(&tmp);

    let db = tmp.join("memory.db");

    let _ = std::fs::remove_file(&db);

    {
        let store = SqliteMemoryStore::open(&db).unwrap();

        store
            .append_record(&rec("a", BASE_MS, WorkloadType::Development))
            .unwrap();

        store
            .append_record(&rec("b", BASE_MS + 1_000, WorkloadType::Gaming))
            .unwrap();
    }

    {
        let store = SqliteMemoryStore::open(&db).unwrap();

        assert_eq!(store.record_count().unwrap(), 2);
        assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_dir(&tmp);
}
