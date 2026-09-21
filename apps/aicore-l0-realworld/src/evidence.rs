use serde::{Deserialize, Serialize};

/// 单场景的验证证据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioEvidence {
    pub scenario: String,
    pub platform: String,
    pub app_version: String,

    // 观察 / 决策
    pub expected_workload: String,
    pub inferred_workload: String,
    pub classifier_confidence: f32,

    // 策略
    pub policy_id: String,
    pub policy_source: String,
    pub policy_reason: String,

    // 执行
    pub overall_status: String,
    pub outcomes_count: usize,

    // 判定
    pub passed: bool,
    pub failures: Vec<String>,
}

/// 一次 L0 验证运行的完整报告。
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
