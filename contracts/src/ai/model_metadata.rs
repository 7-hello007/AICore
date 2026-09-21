use serde::{Deserialize, Serialize};

/// 模型运行时类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntime {
    /// 纯 Rust 特征加权分类器。
    #[default]
    NativeRust,
    /// ONNX Runtime（V3）。
    OnnxRuntime,
    /// llama.cpp（V3）。
    LlamaCpp,
    /// 厂商 SDK（V3）。
    VendorSdk,
    /// Mock（仅用于测试）。
    Mock,
}

/// 隐私等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    /// 强制本地。
    #[default]
    LocalOnly,
    /// 允许本地，可选云端。
    LocalPreferred,
    /// 允许云端（需用户显式开启）。
    CloudAllowed,
}

/// 模型元数据。
///
/// 每个 AI Provider 必须声明自己的元数据，供 Registry 管理。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub runtime: ModelRuntime,
    pub capabilities: Vec<String>,
    /// 延迟预算，毫秒。推理超过此值视为超时。
    pub latency_budget_ms: u64,
    /// 内存预算，MB。
    pub memory_budget_mb: u64,
    pub privacy_class: PrivacyClass,
    /// 模型描述。
    pub description: String,
}

impl ModelMetadata {
    /// 合法性检查。
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.name.trim().is_empty() {
            return Err(AicoreError::ContractInvalid(
                "model name must not be empty".to_string(),
            ));
        }
        if self.version.trim().is_empty() {
            return Err(AicoreError::ContractInvalid(
                "model version must not be empty".to_string(),
            ));
        }
        if self.latency_budget_ms == 0 {
            return Err(AicoreError::ContractInvalid(
                "latency_budget_ms must be > 0".to_string(),
            ));
        }
        if self.latency_budget_ms > 60_000 {
            return Err(AicoreError::ContractInvalid(
                "latency_budget_ms must be <= 60000".to_string(),
            ));
        }
        if self.memory_budget_mb == 0 {
            return Err(AicoreError::ContractInvalid(
                "memory_budget_mb must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// 是否为 local-only。
    pub fn is_local_only(&self) -> bool {
        self.privacy_class == PrivacyClass::LocalOnly
    }
}
