use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::record::HistoricalRecord,
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::InMemoryStore;

fn rec(id: &str, w: WorkloadType, mode: PowerMode) -> HistoricalRecord {
    HistoricalRecord {
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
fn store_appends_and_counts() {
    let store = InMemoryStore::new();
    assert_eq!(store.record_count().unwrap(), 0);

    store
        .append_record(&rec("r1", WorkloadType::Development, PowerMode::Balanced))
        .unwrap();
    store
        .append_record(&rec("r2", WorkloadType::Gaming, PowerMode::Performance))
        .unwrap();

    assert_eq!(store.record_count().unwrap(), 2);
}

#[test]
fn store_clears_history() {
    let store = InMemoryStore::new();
    store
        .append_record(&rec("r1", WorkloadType::Development, PowerMode::Balanced))
        .unwrap();
    store.clear_history().unwrap();
    assert_eq!(store.record_count().unwrap(), 0);
}

#[test]
fn store_reports_name() {
    let store = InMemoryStore::new();
    assert_eq!(store.store_name(), "in_memory");
}

#[test]
fn default_profile_has_privacy_off() {
    let store = InMemoryStore::new();
    let profile = store.load_profile().unwrap();

    assert!(!profile.history_enabled);
    assert!(!profile.profile_enabled);
    assert!(!profile.cloud_inference_allowed);
    assert!(!profile.can_record());
    assert!(!profile.can_personalize());
    assert!(!profile.can_use_cloud());
}

#[test]
fn profile_round_trip() {
    let store = InMemoryStore::new();
    let mut profile = store.load_profile().unwrap();
    profile.history_enabled = true;
    profile.retention_days = 30;
    store.save_profile(&profile).unwrap();

    let loaded = store.load_profile().unwrap();
    assert!(loaded.history_enabled);
    assert_eq!(loaded.retention_days, 30);
}
