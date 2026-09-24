use std::path::PathBuf;

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::record::HistoricalRecord,
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{build_memory_context, InMemoryStore, JsonFileStore};

fn rec(id: &str, w: WorkloadType, mode: PowerMode) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
        timestamp_unix_ms: 1_700_000_000,
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        policy_id: "p-1".to_string(),
        policy_source: PolicySource::RuleBased,
        overall_status: ExecutionOverallStatus::AllSucceeded,
        cpu_usage_percent: 10.0,
        gpu_usage_percent: None,
        npu_usage_percent: None,
        memory_usage_percent: 40.0,
        power_mode: mode,
    }
}

#[test]
fn load_returns_recent_records() {
    let store = InMemoryStore::new();
    for i in 0..5 {
        store
            .append_record(&rec(
                &format!("r{}", i),
                WorkloadType::Development,
                PowerMode::Balanced,
            ))
            .unwrap();
    }
    let loaded = store.load_records(3).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].record_id, "r2");
    assert_eq!(loaded[2].record_id, "r4");
}

#[test]
fn memory_context_dominant_workload() {
    let mut records = Vec::new();
    for _ in 0..3 {
        records.push(rec("d", WorkloadType::Development, PowerMode::Balanced));
    }
    for _ in 0..2 {
        records.push(rec("g", WorkloadType::Gaming, PowerMode::Performance));
    }
    let ctx = build_memory_context(&records);

    assert_eq!(ctx.dominant_workload, Some(WorkloadType::Development));
    assert_eq!(ctx.sample_count, 5);
    assert_eq!(ctx.recent_workloads.len(), 5);
}

#[test]
fn memory_context_empty_is_default() {
    let ctx = build_memory_context(&[]);
    assert!(ctx.dominant_workload.is_none());
    assert!(ctx.preferred_power_mode.is_none());
    assert_eq!(ctx.sample_count, 0);
    assert!(ctx.recent_workloads.is_empty());
}

#[test]
fn memory_context_preferred_power() {
    let mut records = Vec::new();
    for _ in 0..3 {
        records.push(rec("a", WorkloadType::GeneralOffice, PowerMode::PowerSave));
    }
    records.push(rec("b", WorkloadType::Gaming, PowerMode::Performance));

    let ctx = build_memory_context(&records);
    assert_eq!(ctx.preferred_power_mode, Some(PowerMode::PowerSave));
}

#[test]
fn json_file_store_round_trip() {
    let tmp = std::env::temp_dir().join(format!("aicore-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let history = tmp.join("history.json");
    let profile = tmp.join("profile.json");

    // 清理
    let _ = std::fs::remove_file(&history);
    let _ = std::fs::remove_file(&profile);

    {
        let store = JsonFileStore::new(&history, &profile);
        assert_eq!(store.store_name(), "json_file");
        assert_eq!(store.record_count().unwrap(), 0);

        store
            .append_record(&rec("a", WorkloadType::Development, PowerMode::Balanced))
            .unwrap();
        store
            .append_record(&rec("b", WorkloadType::AiInference, PowerMode::Performance))
            .unwrap();

        let mut p = store.load_profile().unwrap();
        p.history_enabled = true;
        store.save_profile(&p).unwrap();
    }

    // 重新打开：验证持久化
    let store = JsonFileStore::new(&history, &profile);
    assert_eq!(store.record_count().unwrap(), 2);
    let loaded = store.load_records(10).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].record_id, "a");
    assert!(store.load_profile().unwrap().history_enabled);

    // 清空
    store.clear_history().unwrap();
    assert_eq!(store.record_count().unwrap(), 0);

    // 清理
    let _ = std::fs::remove_file(&history);
    let _ = std::fs::remove_file(&profile);
    let _: PathBuf = tmp;
}

#[test]
fn json_write_failure_preserves_cached_records_and_profile() {
    let tmp = std::env::temp_dir().join(format!("aicore-json-failure-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let history = tmp.join("history.json");
    let profile = tmp.join("profile.json");
    let store = JsonFileStore::new(&history, &profile);
    store
        .append_record(&rec(
            "saved",
            WorkloadType::Development,
            PowerMode::Balanced,
        ))
        .unwrap();
    let original_profile = store.load_profile().unwrap();
    store.save_profile(&original_profile).unwrap();
    // A directory at the temporary-file path reliably causes a write failure,
    // including when the test runs with elevated privileges.
    std::fs::create_dir(history.with_extension("json.tmp")).unwrap();
    std::fs::create_dir(profile.with_extension("json.tmp")).unwrap();
    assert!(store.clear_history().is_err());
    assert_eq!(store.record_count().unwrap(), 1);
    assert!(store.delete_records_before(u64::MAX).is_err());
    assert_eq!(store.record_count().unwrap(), 1);
    let mut changed_profile = original_profile.clone();
    changed_profile.history_enabled = !original_profile.history_enabled;
    assert!(store.save_profile(&changed_profile).is_err());
    assert_eq!(store.load_profile().unwrap(), original_profile);
    let reopened = JsonFileStore::new(&history, &profile);
    assert_eq!(
        reopened.load_records(10).unwrap(),
        store.load_records(10).unwrap()
    );
    assert_eq!(reopened.load_profile().unwrap(), original_profile);
}

#[test]
fn malformed_json_history_and_profile_are_never_overwritten() {
    let tmp = std::env::temp_dir().join(format!(
        "aicore-json-corrupt-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let history = tmp.join("history.json");
    let profile = tmp.join("profile.json");
    let valid = serde_json::to_string(&rec(
        "valid",
        WorkloadType::Development,
        PowerMode::Balanced,
    ))
    .unwrap();
    let original_history = format!("{valid}\n{{not-json}}\n");
    let original_profile = "{invalid-profile}";
    std::fs::write(&history, &original_history).unwrap();
    std::fs::write(&profile, original_profile).unwrap();

    let store = JsonFileStore::new(&history, &profile);
    assert!(store.load_records(10).is_err());
    assert!(store.record_count().is_err());
    assert!(store
        .append_record(&rec("new", WorkloadType::Development, PowerMode::Balanced))
        .is_err());
    assert!(store.delete_records_before(u64::MAX).is_err());
    assert!(store.clear_history().is_err());
    assert!(store.load_profile().is_err());
    assert!(store.save_profile(&Default::default()).is_err());
    assert_eq!(std::fs::read_to_string(&history).unwrap(), original_history);
    assert_eq!(std::fs::read_to_string(&profile).unwrap(), original_profile);
    std::fs::remove_file(history).unwrap();
    std::fs::remove_file(profile).unwrap();
    std::fs::remove_dir(tmp).unwrap();
}

#[test]
fn legacy_json_array_remains_readable_after_append() {
    let tmp = std::env::temp_dir().join(format!("aicore-json-array-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let history = tmp.join("history.json");
    let profile = tmp.join("profile.json");
    let first = rec("first", WorkloadType::Development, PowerMode::Balanced);
    std::fs::write(&history, serde_json::to_string(&vec![first]).unwrap()).unwrap();
    let store = JsonFileStore::new(&history, &profile);
    store
        .append_record(&rec(
            "second",
            WorkloadType::Development,
            PowerMode::Balanced,
        ))
        .unwrap();
    let reopened = JsonFileStore::new(&history, &profile);
    assert_eq!(reopened.record_count().unwrap(), 2);
    assert_eq!(reopened.load_records(2).unwrap()[1].record_id, "second");
    std::fs::remove_file(history).unwrap();
    std::fs::remove_dir(tmp).unwrap();
}
