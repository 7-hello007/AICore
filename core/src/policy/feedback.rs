use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    effect::{EffectAssessment, EffectComparison, EffectVerdict, OverheadBudget},
    errors::{ContractError, ContractResult},
    memory::context::MemoryContext,
    policy::resource_policy::ResourcePolicy,
    telemetry::snapshot::ComputerSnapshot,
    traits::policy_engine::IPolicyEngine,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Verified regressions suppress the same resource policy shape for that
/// workload. IDs/timestamps are excluded, so evidence survives a new tick ID.
/// This never increases priorities and never bypasses downstream validation.
pub struct FeedbackPolicyEngine {
    primary: Arc<dyn IPolicyEngine>,
    fallback: Arc<dyn IPolicyEngine>,
    regressions: Mutex<HashMap<WorkloadType, (ResourcePolicy, u64)>>,
}

fn same_shape(a: &ResourcePolicy, b: &ResourcePolicy) -> bool {
    a.cpu == b.cpu
        && a.gpu == b.gpu
        && a.npu == b.npu
        && a.memory == b.memory
        && a.power == b.power
        && a.background == b.background
}

impl FeedbackPolicyEngine {
    pub fn new(primary: Arc<dyn IPolicyEngine>, fallback: Arc<dyn IPolicyEngine>) -> Self {
        Self {
            primary,
            fallback,
            regressions: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_effect(
        &self,
        applied: &ResourcePolicy,
        comparison: &EffectComparison,
        budget: OverheadBudget,
        now_unix_ms: u64,
    ) -> ContractResult<EffectAssessment> {
        if applied.policy_id != comparison.policy_id {
            return Err(ContractError::InvalidRequest(
                "effect policy was not applied".into(),
            ));
        }
        let samples = [&comparison.baseline, &comparison.intervention];
        if samples
            .into_iter()
            .chain(comparison.control.iter())
            .any(|s| {
                s.timestamp_unix_ms > now_unix_ms
                    || now_unix_ms.saturating_sub(s.timestamp_unix_ms) > 300_000
            })
            || comparison.intervention.timestamp_unix_ms < comparison.baseline.timestamp_unix_ms
        {
            return Err(ContractError::InvalidRequest(
                "stale, future or unordered effect samples".into(),
            ));
        }
        let assessment = crate::effect::assess_effect(comparison, budget);
        if assessment.verified {
            let fingerprint = applied.clone();
            let mut entries = self
                .regressions
                .lock()
                .map_err(|_| ContractError::LockPoisoned("feedback policy lock poisoned".into()))?;
            let workload = comparison.intervention.workload_type;
            match assessment.verdict {
                EffectVerdict::Regressed => {
                    entries.insert(workload, (fingerprint, now_unix_ms));
                },
                EffectVerdict::Improved
                    if entries
                        .get(&workload)
                        .is_some_and(|(blocked, _)| same_shape(blocked, &fingerprint)) =>
                {
                    entries.remove(&workload);
                },
                _ => {},
            }
        }
        Ok(assessment)
    }

    fn select(
        &self,
        policy: ResourcePolicy,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ResourcePolicy {
        let now = snapshot.collected_at_unix_ms.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(0)
        });
        let blocked = match self.regressions.lock() {
            Ok(entries) => {
                entries
                    .get(&context.workload_type)
                    .is_some_and(|(fingerprint, recorded)| {
                        same_shape(fingerprint, &policy)
                            && now >= *recorded
                            && now - recorded <= 3_600_000
                    })
            },
            Err(_) => true,
        };
        if blocked {
            let mut fallback = self.fallback.generate(snapshot, context);
            fallback
                .reason
                .push_str("; verified-effect regression fallback");
            fallback
        } else {
            policy
        }
    }
}

impl IPolicyEngine for FeedbackPolicyEngine {
    fn generate(&self, snapshot: &ComputerSnapshot, context: &ContextSnapshot) -> ResourcePolicy {
        self.select(self.primary.generate(snapshot, context), snapshot, context)
    }
    fn generate_with_memory(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
        memory: Option<MemoryContext>,
    ) -> ResourcePolicy {
        self.select(
            self.primary.generate_with_memory(snapshot, context, memory),
            snapshot,
            context,
        )
    }
    fn name(&self) -> &'static str {
        "feedback_policy_engine"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::{
        effect::{EffectSample, MeasurementSource},
        policy::priority::PolicyPriority,
    };
    struct Engine(PolicyPriority);
    impl IPolicyEngine for Engine {
        fn generate(&self, _: &ComputerSnapshot, _: &ContextSnapshot) -> ResourcePolicy {
            let mut p = ResourcePolicy::default();
            p.cpu.priority = self.0;
            p.priority = self.0;
            p
        }
        fn name(&self) -> &'static str {
            "test"
        }
    }
    #[test]
    fn observed_regression_changes_next_policy_but_model_does_not() {
        let engine = FeedbackPolicyEngine::new(
            Arc::new(Engine(PolicyPriority::High)),
            Arc::new(Engine(PolicyPriority::Normal)),
        );
        let snapshot = ComputerSnapshot {
            collected_at_unix_ms: Some(100),
            ..Default::default()
        };
        let context = ContextSnapshot::default();
        let policy = engine.generate(&snapshot, &context);
        let sample = |latency| EffectSample {
            source: MeasurementSource::Model,
            timestamp_unix_ms: 90,
            workload_type: context.workload_type,
            latency_ms: Some(latency),
            throughput_per_sec: None,
            energy_mwh: None,
        };
        let mut comparison = EffectComparison {
            policy_id: policy.policy_id.clone(),
            baseline: sample(100.0),
            intervention: sample(130.0),
            control: None,
            control_overhead_ms: 0,
            slow_loop_overhead_ms: 0,
            memory_overhead_mb: 0,
        };
        let budget = OverheadBudget {
            max_control_ms: 10,
            max_slow_ms: 100,
            max_memory_mb: 100,
        };
        assert!(
            !engine
                .record_effect(&policy, &comparison, budget, 100)
                .unwrap()
                .verified
        );
        assert_eq!(
            engine.generate(&snapshot, &context).cpu.priority,
            PolicyPriority::High
        );
        comparison.baseline.source = MeasurementSource::Observed;
        comparison.intervention.source = MeasurementSource::Observed;
        assert_eq!(
            engine
                .record_effect(&policy, &comparison, budget, 100)
                .unwrap()
                .verdict,
            EffectVerdict::Regressed
        );
        assert_eq!(
            engine.generate(&snapshot, &context).cpu.priority,
            PolicyPriority::Normal
        );
        comparison.policy_id = "other".into();
        assert!(engine
            .record_effect(&policy, &comparison, budget, 100)
            .is_err());
    }
}
