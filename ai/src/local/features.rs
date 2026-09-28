use aicore_contracts::{ai::input::AiInput, context::workload::InteractionLevel};

/// Feature vector.
///
/// The dimensions are fixed to allow weight calculation and subsequent replacement with ONNX input.
/// Order: `[cpu, gpu, npu, mem, interactive, passive, idle, has_memory]`
pub const FEATURE_DIM: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureVector {
    pub values: [f32; FEATURE_DIM],
}

impl FeatureVector {
    /// Extract feature from AiInput.
    ///
    /// All values are consolidated to [0.0, 1.0].
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

    /// Point of accumulation.
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
