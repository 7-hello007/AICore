//! Retention Integration Test: Full closed loop for scheduler + real store.

use std::sync::Arc;

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::{profile::UserProfile, record::HistoricalRecord, retention::RetentionPolicy},
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{InMemoryStore, RetentionScheduler, SqliteMemoryStore};

const DAY_MS: u64 = 86_400_000;
const BASE_MS: u64 = 1_700_000_000_000;

fn record(id: &str, ts: u64) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
        timestamp_unix_ms: ts,
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

fn policy() -> RetentionPolicy {
    RetentionPolicy {
        enabled: true,
        check_interval_secs: 60,
        audit_capacity: 10,
    }
}

#[test]
fn end_to_end_with_sqlite_store() {
    let store = SqliteMemoryStore::open_in_memory().unwrap();

    let now = BASE_MS;

    store
        .append_record(&record("ancient", now - 30 * DAY_MS))
        .unwrap();

    store
        .append_record(&record("recent", now - DAY_MS))
        .unwrap();

    let profile = UserProfile {
        history_enabled: true,
        retention_days: 7,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    let store_arc: Arc<dyn IMemoryStore> = Arc::new(store);

    assert_eq!(store_arc.record_count().unwrap(), 2);

    let s = RetentionScheduler::new(store_arc.clone(), policy()).unwrap();
    let report = s.run_once_at(now).unwrap();

    assert!(report.success);
    assert_eq!(report.records_removed, 1);
    assert_eq!(store_arc.record_count().unwrap(), 1);

    let log = s.audit_log();

    assert_eq!(log.len(), 1);
    assert_eq!(log[0].records_removed, 1);
}

#[test]
fn scheduler_repeatedly_runs_at_intervals() {
    let store = Arc::new(InMemoryStore::new());

    store
        .save_profile(&UserProfile {
            retention_days: 7,
            ..Default::default()
        })
        .unwrap();

    let s = RetentionScheduler::new(store, policy()).unwrap();

    assert!(s.should_run(BASE_MS));

    s.run_once_at(BASE_MS).unwrap();

    assert!(!s.should_run(BASE_MS + 30_000));

    assert!(s.should_run(BASE_MS + 60_000));

    s.run_once_at(BASE_MS + 60_000).unwrap();

    assert_eq!(s.audit_log().len(), 2);
    assert_eq!(s.last_run_unix_ms(), BASE_MS + 60_000);
}

#[test]
fn disabled_policy_is_safe_noop() {
    let store = Arc::new(InMemoryStore::new());

    let policy = RetentionPolicy {
        enabled: false,
        ..policy()
    };

    let s = RetentionScheduler::new(store, policy).unwrap();

    for i in 0..10 {
        assert!(!s.should_run(BASE_MS + i * 100_000));
    }

    let report = s.run_once_at(BASE_MS).unwrap();

    assert!(report.did_run() || report.skipped);
}

#[test]
fn store_with_no_profile_uses_defaults() {
    let store = Arc::new(InMemoryStore::new());

    let s = RetentionScheduler::new(store, policy()).unwrap();

    let report = s.run_once_at(BASE_MS).unwrap();

    assert_eq!(report.retention_days, 7);
    assert!(report.is_success());
}

#[test]
fn large_scale_removal() {
    let store = Arc::new(InMemoryStore::new());

    let now = BASE_MS;

    // Article 100: expressly exceeding request window
    for i in 0..100 {
        store
            .append_record(&record(&format!("old-{}", i), now - 30 * DAY_MS - i as u64))
            .unwrap();
    }

    // Article 5: expressly located in the context window
    for i in 0..5 {
        store
            .append_record(&record(&format!("new-{}", i), now - DAY_MS))
            .unwrap();
    }

    store
        .save_profile(&UserProfile {
            retention_days: 7,
            ..Default::default()
        })
        .unwrap();

    let s = RetentionScheduler::new(store.clone(), policy()).unwrap();

    let report = s.run_once_at(now).unwrap();

    assert!(report.success);
    assert_eq!(report.records_removed, 100);
    assert_eq!(store.record_count().unwrap(), 5);
}
