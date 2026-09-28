use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::ai::model_metadata::PrivacyClass;
use aicore_contracts::errors::{ContractError, ContractResult};

use super::entry::ModelEntry;

/// Model registration form.
///
/// Purpose:
/// - Record all available AI Provider.
/// - Query by competency/ privacy level.
/// - Screening based on delayed budget.
pub struct ModelRegistry {
    entries: RwLock<HashMap<String, ModelEntry>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Register model. Overwrite if name already exists.
    pub fn register(&self, entry: ModelEntry) -> ContractResult<()> {
        if let Err(e) = entry.metadata.validate() {
            return Err(ContractError::InvalidRequest(format!(
                "invalid model metadata: {}",
                e
            )));
        }

        let mut entries = self
            .entries
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        entries.insert(entry.metadata.name.clone(), entry);
        Ok(())
    }

    /// Write-off model.
    pub fn unregister(&self, name: &str) -> ContractResult<bool> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.remove(name).is_some())
    }

    /// Query by name.
    pub fn get(&self, name: &str) -> ContractResult<Option<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.get(name).cloned())
    }

    /// List all models.
    pub fn list(&self) -> ContractResult<Vec<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.values().cloned().collect())
    }

    /// Screening by level of privacy.
    pub fn list_by_privacy(&self, class: PrivacyClass) -> ContractResult<Vec<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries
            .values()
            .filter(|e| e.metadata.privacy_class == class)
            .cloned()
            .collect())
    }

    /// Screening by ability.
    pub fn list_by_capability(&self, capability: &str) -> ContractResult<Vec<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries
            .values()
            .filter(|e| e.metadata.capabilities.iter().any(|c| c == capability))
            .cloned()
            .collect())
    }

    /// Statistics.
    pub fn count(&self) -> ContractResult<usize> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.len())
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
