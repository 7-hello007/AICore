use aicore_contracts::ai::model_metadata::ModelMetadata;

/// A model log in Registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEntry {
    pub metadata: ModelMetadata,
    /// Provider name.
    pub provider_name: String,
    /// Current availability.
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
