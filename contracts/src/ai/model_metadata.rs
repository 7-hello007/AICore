use serde::{Deserialize, Serialize};

/// model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntime {
    /// Pure Rust feature-weighted classifier.
    #[default]
    NativeRust,
    /// ONNX Runtime（V3）。
    OnnxRuntime,
    /// llama.cpp（V3）。
    LlamaCpp,
    /// Vendor SDK (V3).
    VendorSdk,
    /// Mock (for testing only).
    Mock,
}

/// Privacy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    /// Force local.
    #[default]
    LocalOnly,
    /// Allow local, optional cloud.
    LocalPreferred,
    /// Allows clouds to be opened (user visible).
    CloudAllowed,
}

/// Model metadata.
///
/// Each AI Provider must declare its metadata for Registry management.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub runtime: ModelRuntime,
    pub capabilities: Vec<String>,
    /// Delays in budget, milliseconds.
    pub latency_budget_ms: u64,
    /// Memory budget, MB.
    pub memory_budget_mb: u64,
    pub privacy_class: PrivacyClass,
    /// Model description.
    pub description: String,
}

impl ModelMetadata {
    /// Legitimacy check.
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

    /// Is it local-only.
    pub fn is_local_only(&self) -> bool {
        self.privacy_class == PrivacyClass::LocalOnly
    }
}
