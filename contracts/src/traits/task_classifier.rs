use crate::context::workload::WorkloadType;

/// 分类器输入。纯数据，不依赖任何 core 类型。
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

/// workload 分类器接口。
///
/// 实现可以是规则、ML 模型、LLM。Core 只依赖这个接口。
pub trait ITaskClassifier: Send + Sync {
    fn classify(&self, input: &ClassificationInput) -> WorkloadType;

    /// 返回分类置信度。实现可以覆盖；默认值仅作为兼容兜底。
    fn confidence(&self, input: &ClassificationInput) -> f32 {
        match self.classify(input) {
            WorkloadType::Unknown => 0.0,
            _ => 0.6,
        }
    }

    fn name(&self) -> &'static str;
}
