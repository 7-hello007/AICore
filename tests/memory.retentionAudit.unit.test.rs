//! Retention 审计日志测试。

use std::sync::Arc;

use aicore_contracts::{
    memory::{profile::UserProfile, retention::RetentionPolicy},
    traits::memory_store::IMemoryStore,
};
use aicore_memory::{InMemoryStore, RetentionScheduler};

fn policy(capacity: usize) -> RetentionPolicy {
    RetentionPolicy {
        enabled: true,
        check_interval_secs: 1,
        audit_capacity: capacity,
    }
}

#[test]
fn audit_starts_empty() {
    let store = Arc::new(InMemoryStore::new());
    let s = RetentionScheduler::new(store, policy(10)).unwrap();
    assert!(s.audit_log().is_empty());
}

#[test]
fn audit_grows_with_runs() {
    let store = Arc::new(InMemoryStore::new());
    let s = RetentionScheduler::new(store, policy(10)).unwrap();

    for i in 0..3 {
        s.run_once_at(1000 + i).unwrap();
    }

    assert_eq!(s.audit_log().len(), 3);
}

#[test]
fn audit_respects_capacity() {
    let store = Arc::new(InMemoryStore::new());
    let s = RetentionScheduler::new(store, policy(2)).unwrap();

    for i in 0..5 {
        s.run_once_at(1000 + i).unwrap();
    }

    let log = s.audit_log();
    assert_eq!(log.len(), 2, "ring buffer should cap at 2");
    // 最新的两条
    assert_eq!(log[0].ran_at_unix_ms, 1003);
    assert_eq!(log[1].ran_at_unix_ms, 1004);
}

#[test]
fn audit_entries_are_ordered_oldest_to_newest() {
    let store = Arc::new(InMemoryStore::new());
    let s = RetentionScheduler::new(store, policy(10)).unwrap();

    s.run_once_at(100).unwrap();
    s.run_once_at(200).unwrap();
    s.run_once_at(300).unwrap();

    let log = s.audit_log();
    assert_eq!(log[0].ran_at_unix_ms, 100);
    assert_eq!(log[1].ran_at_unix_ms, 200);
    assert_eq!(log[2].ran_at_unix_ms, 300);
}

#[test]
fn audit_records_failures() {
    // 用一个永远失败的 store 模拟
    struct FailStore;
    impl IMemoryStore for FailStore {
        fn append_record(
            &self,
            _r: &aicore_contracts::memory::record::HistoricalRecord,
        ) -> aicore_contracts::errors::ContractResult<()> {
            Ok(())
        }
        fn load_records(
            &self,
            _l: usize,
        ) -> aicore_contracts::errors::ContractResult<
            Vec<aicore_contracts::memory::record::HistoricalRecord>,
        > {
            Ok(vec![])
        }
        fn clear_history(&self) -> aicore_contracts::errors::ContractResult<()> {
            Ok(())
        }
        fn record_count(&self) -> aicore_contracts::errors::ContractResult<usize> {
            Ok(0)
        }
        fn load_profile(&self) -> aicore_contracts::errors::ContractResult<UserProfile> {
            Err(aicore_contracts::errors::ContractError::AdapterUnavailable(
                "simulated".to_string(),
            ))
        }
        fn save_profile(&self, _p: &UserProfile) -> aicore_contracts::errors::ContractResult<()> {
            Ok(())
        }
        fn store_name(&self) -> &'static str {
            "fail_store"
        }
    }

    let store: Arc<dyn IMemoryStore> = Arc::new(FailStore);
    let s = RetentionScheduler::new(store, policy(10)).unwrap();
    let report = s.run_once_at(1000).unwrap();

    assert!(!report.success);
    assert!(!report.skipped);
    assert!(report.message.as_ref().unwrap().contains("load_profile"));

    let log = s.audit_log();
    assert_eq!(log.len(), 1);
    assert!(!log[0].success);
}

#[test]
fn clear_audit_empties_log() {
    let store = Arc::new(InMemoryStore::new());
    let s = RetentionScheduler::new(store, policy(10)).unwrap();
    s.run_once_at(1000).unwrap();
    assert_eq!(s.audit_log().len(), 1);

    s.clear_audit();
    assert!(s.audit_log().is_empty());
}
