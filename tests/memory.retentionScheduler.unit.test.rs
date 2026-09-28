//! RetentionScheduler unit test.

use std::sync::Arc;

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    execution::status::ExecutionOverallStatus,
    memory::{profile::UserProfile, record::HistoricalRecord, retention::RetentionPolicy},
    policy::{priority::PolicySource, resource_policy::PowerMode},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{InMemoryStore, RetentionScheduler};

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

fn enabled_policy() -> RetentionPolicy {
    RetentionPolicy {
        enabled: true,
        check_interval_secs: 60,
        audit_capacity: 5,
    }
}

#[test]
fn disabled_policy_never_runs() {
    let store = Arc::new(InMemoryStore::new());
    let policy = RetentionPolicy {
        enabled: false,
        ..enabled_policy()
    };
    let s = RetentionScheduler::new(store, policy).unwrap();
    assert!(!s.should_run(1_000_000));
}

#[test]
fn first_run_is_immediate() {
    let store = Arc::new(InMemoryStore::new());

    let s = RetentionScheduler::new(store, enabled_policy()).unwrap();

    assert!(s.should_run(BASE_MS));
    assert_eq!(s.last_run_unix_ms(), 0);
}

#[test]
fn respects_check_interval() {
    let store = Arc::new(InMemoryStore::new());

    let s = RetentionScheduler::new(store, enabled_policy()).unwrap();

    s.run_once_at(BASE_MS).unwrap();

    assert!(!s.should_run(BASE_MS + 30_000));

    assert!(s.should_run(BASE_MS + 60_000));

    assert!(s.should_run(BASE_MS + 100_000));
}

#[test]
fn retention_days_zero_skips() {
    let store = Arc::new(InMemoryStore::new());

    let profile = UserProfile {
        retention_days: 0,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    let s = RetentionScheduler::new(store, enabled_policy()).unwrap();

    let report = s.run_once_at(BASE_MS).unwrap();

    assert!(report.skipped);
    assert!(report.message.as_ref().unwrap().contains("unlimited"));
}

#[test]
fn removes_old_records() {
    let store = Arc::new(InMemoryStore::new());

    let now = BASE_MS;

    store
        .append_record(&record("old", now - 30 * DAY_MS))
        .unwrap();

    store.append_record(&record("new", now - DAY_MS)).unwrap();

    let profile = UserProfile {
        retention_days: 7,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    assert_eq!(store.record_count().unwrap(), 2);

    let s = RetentionScheduler::new(store.clone(), enabled_policy()).unwrap();

    let report = s.run_once_at(now).unwrap();

    assert!(report.success);
    assert_eq!(report.records_removed, 1);
    assert_eq!(store.record_count().unwrap(), 1);
}

#[test]
fn report_metadata_correct() {
    let store = Arc::new(InMemoryStore::new());

    let profile = UserProfile {
        retention_days: 14,
        ..Default::default()
    };

    store.save_profile(&profile).unwrap();

    let s = RetentionScheduler::new(store, enabled_policy()).unwrap();

    let report = s.run_once_at(BASE_MS).unwrap();

    assert_eq!(report.ran_at_unix_ms, BASE_MS);
    assert_eq!(report.retention_days, 14);
    assert!(report.is_success());
    assert!(report.did_run());
}

#[test]
fn last_run_updated_even_on_skip() {
    let store = Arc::new(InMemoryStore::new());

    let s = RetentionScheduler::new(store, enabled_policy()).unwrap();

    s.run_once_at(BASE_MS).unwrap();

    assert_eq!(s.last_run_unix_ms(), BASE_MS);
}

#[test]
fn invalid_policy_rejected() {
    let store = Arc::new(InMemoryStore::new());
    let bad = RetentionPolicy {
        enabled: true,
        check_interval_secs: 0,
        audit_capacity: 5,
    };
    assert!(RetentionScheduler::new(store.clone(), bad).is_err());

    let bad2 = RetentionPolicy {
        enabled: true,
        check_interval_secs: 60,
        audit_capacity: 0,
    };
    assert!(RetentionScheduler::new(store, bad2).is_err());
}

#[test]
fn default_policy_is_disabled() {
    let p = RetentionPolicy::default();
    assert!(!p.enabled);
    assert_eq!(p.check_interval_secs, 3600);
    assert_eq!(p.audit_capacity, 100);
    assert!(p.validate().is_ok());
}
