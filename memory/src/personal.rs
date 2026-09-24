use aicore_contracts::{
    errors::ContractResult, memory::context::MemoryContext, traits::memory_store::IMemoryStore,
};

/// Builds an aggregate only when the current profile permits personalization.
/// `history_enabled` controls new writes independently of this read consent.
pub fn personalized_context(
    store: &dyn IMemoryStore,
    limit: usize,
) -> ContractResult<Option<MemoryContext>> {
    let profile = store.load_profile()?;
    if !profile.can_personalize() {
        return Ok(None);
    }
    let records = store.load_records(limit)?;
    if records.is_empty() {
        return Ok(None);
    }
    Ok(Some(MemoryContext::from_records(&records)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryStore;
    use aicore_contracts::{
        context::workload::{InteractionLevel, WorkloadType},
        execution::status::ExecutionOverallStatus,
        memory::{profile::UserProfile, record::HistoricalRecord},
        policy::{priority::PolicySource, resource_policy::PowerMode},
    };

    fn record(id: &str, timestamp_unix_ms: u64) -> HistoricalRecord {
        HistoricalRecord {
            application_id: None,
            record_id: id.into(),
            timestamp_unix_ms,
            workload_type: WorkloadType::Development,
            interaction_level: InteractionLevel::Interactive,
            policy_id: "policy".into(),
            policy_source: PolicySource::RuleBased,
            overall_status: ExecutionOverallStatus::AllSucceeded,
            cpu_usage_percent: 10.0,
            gpu_usage_percent: None,
            npu_usage_percent: None,
            memory_usage_percent: 20.0,
            power_mode: PowerMode::Balanced,
        }
    }

    #[test]
    fn history_consent_alone_never_exposes_personal_context() {
        let store = InMemoryStore::new();
        store.append_record(&record("one", 1)).unwrap();
        store
            .save_profile(&UserProfile {
                history_enabled: true,
                profile_enabled: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(personalized_context(&store, 100).unwrap(), None);
        store
            .save_profile(&UserProfile {
                history_enabled: true,
                profile_enabled: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            personalized_context(&store, 100)
                .unwrap()
                .unwrap()
                .sample_count,
            1
        );
        store.save_profile(&UserProfile::default()).unwrap();
        assert_eq!(personalized_context(&store, 100).unwrap(), None);
        assert_eq!(store.record_count().unwrap(), 1);
        store.clear_history().unwrap();
        assert_eq!(store.record_count().unwrap(), 0);
    }

    #[test]
    fn retention_removes_old_samples_before_aggregation() {
        let store = InMemoryStore::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        store.append_record(&record("old", 1)).unwrap();
        store.append_record(&record("new", now)).unwrap();
        store
            .save_profile(&UserProfile {
                profile_enabled: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(store.apply_retention(7).unwrap(), 1);
        assert_eq!(
            personalized_context(&store, 100)
                .unwrap()
                .unwrap()
                .sample_count,
            1
        );
    }
}
