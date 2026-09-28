use crate::context::workload::WorkloadType;

/// . Pure data, not dependent on any core type.
#[derive(Debug, Clone, Default)]
pub struct ClassificationInput {
    pub cpu_usage_percent: f32,
    pub gpu_usage_percent: Option<f32>,
    pub npu_usage_percent: Option<f32>,
    pub memory_usage_percent: f32,
    pub foreground_app: Option<String>,
    pub foreground_process: Option<String>,
    pub foreground_window: Option<String>,
    pub active_process_names: Vec<String>,
}

/// Workload classification interface.
///
/// This can be the rule, ML model, LLM. Core relies only on this interface.
pub trait ITaskClassifier: Send + Sync {
    fn classify(&self, input: &ClassificationInput) -> WorkloadType;

    /// Returns the class confidence. Accomplishment can be overridden; the default value is only used as a compatible loop.
    fn confidence(&self, input: &ClassificationInput) -> f32 {
        match self.classify(input) {
            WorkloadType::Unknown => 0.0,
            _ => 0.6,
        }
    }

    fn name(&self) -> &'static str;
}
