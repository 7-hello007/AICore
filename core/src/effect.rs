use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    context::workload::WorkloadType,
    effect::{
        EffectAssessment, EffectComparison, EffectSample, EffectVerdict, MeasurementSource,
        OverheadBudget,
    },
    errors::{ContractError, ContractResult},
};

/// Measure a caller-owned workload completion with a monotonic clock. Only
/// latency is observed here; throughput and energy remain unavailable unless
/// an independent collector supplies them. A failed workload yields no sample.
pub fn observe_workload<T>(
    workload_type: WorkloadType,
    run: impl FnOnce() -> ContractResult<T>,
) -> ContractResult<(T, EffectSample)> {
    let started = Instant::now();
    let output = run()?;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    let timestamp_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ContractError::ResourceUnavailable(format!("system clock: {error}")))?
        .as_millis()
        .min(u64::MAX as u128) as u64;
    Ok((
        output,
        EffectSample {
            source: MeasurementSource::Observed,
            timestamp_unix_ms,
            workload_type,
            latency_ms: Some(latency_ms),
            throughput_per_sec: None,
            energy_mwh: None,
        },
    ))
}

/// Evaluates measured effects without turning simulated or absent readings
/// into an observed improvement. The optional control sample is a concurrent
/// reference; without it the before sample is used as the reference.
pub fn assess_effect(comparison: &EffectComparison, budget: OverheadBudget) -> EffectAssessment {
    let budget_met = budget.contains(comparison);
    let reference = comparison.control.as_ref().unwrap_or(&comparison.baseline);
    let valid_sources = [&comparison.baseline, &comparison.intervention, reference]
        .iter()
        .all(|sample| sample.source == MeasurementSource::Observed);
    let valid_workload = comparison.baseline.workload_type == comparison.intervention.workload_type
        && reference.workload_type == comparison.intervention.workload_type;
    let mut result = EffectAssessment {
        policy_id: comparison.policy_id.clone(),
        verdict: EffectVerdict::Inconclusive,
        verified: false,
        latency_improvement_percent: None,
        throughput_improvement_percent: None,
        energy_improvement_percent: None,
        budget_met,
        reason: String::new(),
    };
    if !valid_sources || !valid_workload {
        result.reason = "samples are simulated or workloads differ".into();
        return result;
    }

    result.latency_improvement_percent = improvement(
        reference.latency_ms,
        comparison.intervention.latency_ms,
        true,
    );
    result.throughput_improvement_percent = improvement(
        reference.throughput_per_sec,
        comparison.intervention.throughput_per_sec,
        false,
    );
    result.energy_improvement_percent = improvement(
        reference.energy_mwh,
        comparison.intervention.energy_mwh,
        true,
    );
    let values: Vec<f64> = [
        result.latency_improvement_percent,
        result.throughput_improvement_percent,
        result.energy_improvement_percent,
    ]
    .into_iter()
    .flatten()
    .collect();
    if values.is_empty() {
        result.reason = "no comparable observed metric".into();
        return result;
    }
    result.verified = true;
    result.verdict = if !budget_met || values.iter().any(|value| *value < -1.0) {
        EffectVerdict::Regressed
    } else if values.iter().any(|value| *value > 1.0) {
        EffectVerdict::Improved
    } else {
        EffectVerdict::Inconclusive
    };
    result.reason = if !budget_met {
        "control, slow-loop or memory overhead exceeded budget".into()
    } else {
        "paired observed metrics assessed against baseline/control".into()
    };
    result
}

fn improvement(
    reference: Option<f64>,
    intervention: Option<f64>,
    lower_is_better: bool,
) -> Option<f64> {
    let (reference, intervention) = (reference?, intervention?);
    if !reference.is_finite() || !intervention.is_finite() || reference <= 0.0 || intervention < 0.0
    {
        return None;
    }
    let delta = if lower_is_better {
        reference - intervention
    } else {
        intervention - reference
    };
    Some(100.0 * delta / reference)
}

/// Bounded feedback of verified observations. Policy engines can consume this
/// summary, but the tracker never changes an active policy by itself.
pub struct EffectFeedbackTracker {
    assessments: Mutex<VecDeque<EffectAssessment>>,
    capacity: usize,
}

impl EffectFeedbackTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            assessments: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
        }
    }

    pub fn record(&self, assessment: EffectAssessment) -> ContractResult<bool> {
        if !assessment.verified {
            return Ok(false);
        }
        let mut assessments = self
            .assessments
            .lock()
            .map_err(|_| ContractError::LockPoisoned("effect feedback lock poisoned".into()))?;
        assessments.push_back(assessment);
        while assessments.len() > self.capacity {
            assessments.pop_front();
        }
        Ok(true)
    }

    pub fn for_policy(&self, policy_id: &str) -> ContractResult<Vec<EffectAssessment>> {
        let assessments = self
            .assessments
            .lock()
            .map_err(|_| ContractError::LockPoisoned("effect feedback lock poisoned".into()))?;
        Ok(assessments
            .iter()
            .filter(|assessment| assessment.policy_id == policy_id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::context::workload::WorkloadType;
    use aicore_contracts::effect::EffectSample;

    fn sample(source: MeasurementSource, latency: f64) -> EffectSample {
        EffectSample {
            source,
            timestamp_unix_ms: 1,
            workload_type: WorkloadType::Development,
            latency_ms: Some(latency),
            throughput_per_sec: None,
            energy_mwh: None,
        }
    }

    fn budget() -> OverheadBudget {
        OverheadBudget {
            max_control_ms: 10,
            max_slow_ms: 100,
            max_memory_mb: 32,
        }
    }

    #[test]
    fn isolated_workload_records_only_measured_latency() {
        let (first, sample) = observe_workload(WorkloadType::Development, || {
            let mut values: Vec<u32> = (0..100_000).rev().collect();
            values.sort_unstable();
            Ok(values[0])
        })
        .unwrap();
        assert_eq!(first, 0);
        assert_eq!(sample.source, MeasurementSource::Observed);
        assert!(sample.timestamp_unix_ms > 0);
        assert!(sample
            .latency_ms
            .is_some_and(|value| value.is_finite() && value > 0.0));
        assert_eq!(sample.throughput_per_sec, None);
        assert_eq!(sample.energy_mwh, None);

        let failed: ContractResult<((), EffectSample)> =
            observe_workload(WorkloadType::Development, || {
                Err(ContractError::InvalidRequest("workload failed".into()))
            });
        assert!(matches!(failed, Err(ContractError::InvalidRequest(_))));
    }

    #[test]
    fn observed_control_comparison_generates_feedback() {
        let comparison = EffectComparison {
            policy_id: "policy".into(),
            baseline: sample(MeasurementSource::Observed, 100.0),
            intervention: sample(MeasurementSource::Observed, 70.0),
            control: Some(sample(MeasurementSource::Observed, 90.0)),
            control_overhead_ms: 2,
            slow_loop_overhead_ms: 20,
            memory_overhead_mb: 4,
        };
        let assessment = assess_effect(&comparison, budget());
        assert!(assessment.verified);
        assert_eq!(assessment.verdict, EffectVerdict::Improved);
        assert!((assessment.latency_improvement_percent.unwrap() - 22.222).abs() < 0.01);
        let tracker = EffectFeedbackTracker::new(2);
        assert!(tracker.record(assessment).unwrap());
        assert_eq!(tracker.for_policy("policy").unwrap().len(), 1);
    }

    #[test]
    fn simulated_or_missing_data_cannot_claim_verified_gain() {
        let comparison = EffectComparison {
            policy_id: "policy".into(),
            baseline: sample(MeasurementSource::Model, 100.0),
            intervention: sample(MeasurementSource::Model, 1.0),
            control: None,
            control_overhead_ms: 0,
            slow_loop_overhead_ms: 0,
            memory_overhead_mb: 0,
        };
        let assessment = assess_effect(&comparison, budget());
        assert!(!assessment.verified);
        assert_eq!(assessment.verdict, EffectVerdict::Inconclusive);
        assert!(!EffectFeedbackTracker::new(1).record(assessment).unwrap());
    }

    #[test]
    fn overhead_violation_is_regression_even_if_latency_improves() {
        let comparison = EffectComparison {
            policy_id: "policy".into(),
            baseline: sample(MeasurementSource::Observed, 100.0),
            intervention: sample(MeasurementSource::Observed, 50.0),
            control: None,
            control_overhead_ms: 50,
            slow_loop_overhead_ms: 20,
            memory_overhead_mb: 4,
        };
        let assessment = assess_effect(&comparison, budget());
        assert!(assessment.verified);
        assert!(!assessment.budget_met);
        assert_eq!(assessment.verdict, EffectVerdict::Regressed);
    }
}
