use serde::{Deserialize, Serialize};

/// 模型种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// 特征加权分类器（纯 Rust）。
    FeatureWeighted,
    /// ONNX Runtime 模型（V3.2）。
    Onnx,
    /// llama.cpp GGUF 模型（V3.2 或更晚）。
    LlamaCpp,
}

/// 张量数据类型。
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

/// 张量 schema。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TensorSchema {
    pub name: String,
    /// 形状，`-1` 表示动态维度。
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

/// ONNX 模型专用声明。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnnxModelSpec {
    pub input_schema: Vec<TensorSchema>,
    pub output_schema: Vec<TensorSchema>,
    /// 可选的 execution provider 偏好。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_provider: Option<String>,
}

/// 模型来源。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSource {
    File { path: String },
    Inline { json: String },
}

/// 模型声明。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub name: String,
    pub version: String,
    pub kind: ModelKind,
    pub source: ModelSource,
    /// ONNX 专用声明；仅 `kind == Onnx` 时使用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onnx_spec: Option<OnnxModelSpec>,
}

impl ModelSpec {
    /// 校验 kind 与专用声明的匹配。
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

/// 加载后的模型句柄。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelHandle {
    pub handle_id: String,
    pub model_name: String,
    pub model_version: String,
    pub loaded_at_unix_ms: u64,
}

/// 推理请求。
///
/// V3.2：保留 `features` 作为简单输入；`tensors` 用于 ONNX 多输入。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InferenceRequest {
    pub features: Vec<f32>,
    /// ONNX 多输入张量（按 `input_schema` 顺序）。
    pub tensors: Vec<TensorInput>,
}

/// 单个张量输入。
#[derive(Debug, Clone, PartialEq)]
pub struct TensorInput {
    pub name: String,
    pub shape: Vec<i64>,
    pub data: Vec<f32>,
}

/// 单个标签的推理分数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceLabel {
    pub label: String,
    pub score: f32,
}

/// 推理响应。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub labels: Vec<InferenceLabel>,
    pub latency_ms: u64,
    pub model_version: String,
}

/// Runtime 能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    LoadFromFile,
    LoadFromMemory,
    FeatureInference,
    OnnxInference,
    LlamaCppInference,
}

/// Runtime 健康状态。
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeHealth {
    Healthy { loaded_models: usize },
    Degraded { reason: String },
    NotReady,
}
