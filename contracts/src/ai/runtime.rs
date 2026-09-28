use serde::{Deserialize, Serialize};

/// Model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Feature-weighted classifier (pure Rust).
    FeatureWeighted,
    /// ONNX Runtime model (V3.2).
    Onnx,
    /// llama.cpp GGUF model (V3.2 or later).
    LlamaCpp,
}

/// Tensor data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorDType {
    F32,
    F16,
    I64,
    U8,
}

impl TensorDType {
    pub fn element_size(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
            Self::I64 => 8,
            Self::U8 => 1,
        }
    }
}

/// Tensor schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorSchema {
    pub name: String,
    /// shape, `-1` means dynamic dimensions.
    pub shape: Vec<i64>,
    pub dtype: TensorDType,
}

impl TensorSchema {
    pub fn rank(&self) -> usize {
        self.shape.len()
    }

    pub fn element_count(&self) -> Option<usize> {
        if self.shape.iter().any(|d| *d < 0) {
            return None;
        }
        Some(self.shape.iter().map(|d| *d as usize).product())
    }
}

/// ONNX-specific model declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnnxModelSpec {
    pub input_schema: Vec<TensorSchema>,
    pub output_schema: Vec<TensorSchema>,
    /// Optional execution-provider preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_provider: Option<String>,
}

/// Model source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSource {
    File { path: String },
    Inline { json: String },
}

/// Model declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub name: String,
    pub version: String,
    pub kind: ModelKind,
    pub source: ModelSource,
    /// ONNX-specific declaration, used only when `kind == Onnx`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onnx_spec: Option<OnnxModelSpec>,
}

impl ModelSpec {
    /// Validate that the kind matches its specialized declaration.
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.name.trim().is_empty() {
            return Err(AicoreError::ContractInvalid("name is empty".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(AicoreError::ContractInvalid("version is empty".to_string()));
        }
        match self.kind {
            ModelKind::Onnx if self.onnx_spec.is_none() => Err(AicoreError::ContractInvalid(
                "Onnx model requires onnx_spec".to_string(),
            )),
            ModelKind::FeatureWeighted if self.onnx_spec.is_some() => {
                Err(AicoreError::ContractInvalid(
                    "FeatureWeighted model must not set onnx_spec".to_string(),
                ))
            },
            _ => Ok(()),
        }
    }
}

/// A loaded model handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelHandle {
    pub handle_id: String,
    pub model_name: String,
    pub model_version: String,
    pub loaded_at_unix_ms: u64,
}

/// Inference request.
///
/// V3.2 retains `features` for simple input; `tensors` supports multiple ONNX inputs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InferenceRequest {
    pub features: Vec<f32>,
    /// ONNX input tensors, in `input_schema` order.
    pub tensors: Vec<TensorInput>,
}

/// Single tensor input.
#[derive(Debug, Clone, PartialEq)]
pub struct TensorInput {
    pub name: String,
    pub shape: Vec<i64>,
    pub data: Vec<f32>,
}

/// Inference score for one label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceLabel {
    pub label: String,
    pub score: f32,
}

/// Inference response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub labels: Vec<InferenceLabel>,
    pub latency_ms: u64,
    pub model_version: String,
}

/// Runtime capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    LoadFromFile,
    LoadFromMemory,
    FeatureInference,
    OnnxInference,
    LlamaCppInference,
}

/// Runtime health state.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeHealth {
    Healthy { loaded_models: usize },
    Degraded { reason: String },
    NotReady,
}
