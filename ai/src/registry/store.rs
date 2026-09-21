use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::ai::model_metadata::PrivacyClass;
use aicore_contracts::errors::{ContractError, ContractResult};

use super::entry::ModelEntry;

/// 模型注册表。
///
/// 用途：
/// - 记录所有可用 AI Provider。
/// - 按能力 / 隐私等级查询。
/// - 按延迟预算筛选。
pub struct ModelRegistry {
    entries: RwLock<HashMap<String, ModelEntry>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// 注册模型。若 name 已存在则覆盖。
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

    /// 注销模型。
    pub fn unregister(&self, name: &str) -> ContractResult<bool> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.remove(name).is_some())
    }

    /// 按名称查询。
    pub fn get(&self, name: &str) -> ContractResult<Option<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.get(name).cloned())
    }

    /// 列出所有模型。
    pub fn list(&self) -> ContractResult<Vec<ModelEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(entries.values().cloned().collect())
    }

    /// 按隐私等级筛选。
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

    /// 按能力筛选。
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

    /// 统计。
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
