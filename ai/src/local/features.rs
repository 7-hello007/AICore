use aicore_contracts::{ai::input::AiInput, context::workload::InteractionLevel};

/// 特征向量。
///
/// 维度固定，便于权重计算与后续替换为 ONNX 输入。
/// 顺序：`[cpu, gpu, npu, mem, interactive, passive, idle, has_memory]`
pub const FEATURE_DIM: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureVector {
    pub values: [f32; FEATURE_DIM],
}

impl FeatureVector {
    /// 从 AiInput 提取特征。
    ///
    /// 所有值归一化到 [0.0, 1.0]。
    pub fn from_input(input: &AiInput) -> Self {
        let cpu = clamp01(input.snapshot.cpu.usage_percent / 100.0);
        let gpu = clamp01(input.snapshot.gpu.usage_percent.unwrap_or(0.0) / 100.0);
        let npu = clamp01(input.snapshot.npu.usage_percent.unwrap_or(0.0) / 100.0);
        let mem = clamp01(input.snapshot.memory.usage_percent / 100.0);

        let (interactive, passive, idle) = match input.context.interaction_level {
            InteractionLevel::Interactive => (1.0, 0.0, 0.0),
            InteractionLevel::Passive => (0.0, 1.0, 0.0),
            InteractionLevel::Idle => (0.0, 0.0, 1.0),
        };

        let has_memory = if input.memory.is_some() { 1.0 } else { 0.0 };

        Self {
            values: [cpu, gpu, npu, mem, interactive, passive, idle, has_memory],
        }
    }

    /// 点积。
    pub fn dot(&self, weights: &[f32; FEATURE_DIM]) -> f32 {
        self.values
            .iter()
            .zip(weights.iter())
            .map(|(a, b)| a * b)
            .sum()
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}
