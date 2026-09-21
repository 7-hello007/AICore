use aicore_contracts::ai::model_metadata::ModelMetadata;

/// Registry 中的一条模型记录。
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEntry {
    pub metadata: ModelMetadata,
    /// provider 名称。
    pub provider_name: String,
    /// 当前是否可用。
    pub available: bool,
}

impl ModelEntry {
    pub fn new(metadata: ModelMetadata, provider_name: impl Into<String>, available: bool) -> Self {
        Self {
            metadata,
            provider_name: provider_name.into(),
            available,
        }
    }
}
