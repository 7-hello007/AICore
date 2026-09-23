use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementSource {
    Observed,
    Model,
}

/// A measurement of one workload interval. Missing fields stay missing; zero
/// is not used as a substitute for unavailable hardware readings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectSample {
    pub source: MeasurementSource,
    pub timestamp_unix_ms: u64,
    pub workload_type: WorkloadType,
    pub latency_ms: Option<f64>,
    pub throughput_per_sec: Option<f64>,
    pub energy_mwh: Option<f64>,
}

/// Paired baseline/control and intervention observations of the same workload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectComparison {
    pub policy_id: String,
    pub baseline: EffectSample,
    pub intervention: EffectSample,
    pub control: Option<EffectSample>,
    pub control_overhead_ms: u64,
    pub slow_loop_overhead_ms: u64,
    pub memory_overhead_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectVerdict {
    Improved,
    Regressed,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectAssessment {
    pub policy_id: String,
    pub verdict: EffectVerdict,
    pub verified: bool,
    pub latency_improvement_percent: Option<f64>,
    pub throughput_improvement_percent: Option<f64>,
    pub energy_improvement_percent: Option<f64>,
    pub budget_met: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverheadBudget {
    pub max_control_ms: u64,
    pub max_slow_ms: u64,
    pub max_memory_mb: u64,
}

impl OverheadBudget {
    pub fn contains(&self, comparison: &EffectComparison) -> bool {
        comparison.control_overhead_ms <= self.max_control_ms
            && comparison.slow_loop_overhead_ms <= self.max_slow_ms
            && comparison.memory_overhead_mb <= self.max_memory_mb
    }
}
