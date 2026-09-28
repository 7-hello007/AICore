use serde::{Deserialize, Serialize};

/// Proof of the single scene.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioEvidence {
    pub scenario: String,
    pub platform: String,
    pub app_version: String,

    // Observation / Decision-making
    pub expected_workload: String,
    pub inferred_workload: String,
    pub classifier_confidence: f32,

    // Policy
    pub policy_id: String,
    pub policy_source: String,
    pub policy_reason: String,

    // Implementation
    pub overall_status: String,
    pub outcomes_count: usize,

    // Decision
    pub passed: bool,
    pub failures: Vec<String>,
}

/// A full report of the L0 validation operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RealWorldReport {
    pub version: String,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub scenarios_run: usize,
    pub scenarios_passed: usize,
    pub scenarios_failed: usize,
    pub evidences: Vec<ScenarioEvidence>,
}

impl RealWorldReport {
    pub fn all_passed(&self) -> bool {
        self.scenarios_failed == 0 && self.scenarios_run > 0
    }
}
