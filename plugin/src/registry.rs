use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::plugin::{
    api_version::ApiVersion,
    manifest::{PluginKind, PluginManifest, TrustLevel},
    state::PluginState,
};
use serde::{Deserialize, Serialize};

/// Registry 中的一条插件记录。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub state: PluginState,
    /// 上次状态变更原因。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reason: Option<String>,
    /// 连续失败次数。
    #[serde(default)]
    pub failure_count: u32,
}

impl PluginEntry {
    pub fn new(manifest: PluginManifest) -> Self {
        Self {
            manifest,
            state: PluginState::Discovered,
            last_reason: None,
            failure_count: 0,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.last_reason = Some(reason.into());
        self
    }
}

/// 插件注册表。
///
/// 线程安全。所有状态变更通过 `transition` 完成，强制状态机规则。
pub struct PluginRegistry {
    entries: RwLock<HashMap<String, PluginEntry>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个新插件（初始状态 Discovered）。
    ///
    /// 若 name 已存在则拒绝，避免覆盖已激活的插件。
    pub fn register(&self, manifest: PluginManifest) -> Result<(), RegistryError> {
        manifest
            .validate()
            .map_err(|e| RegistryError::InvalidManifest(e.to_string()))?;

        if !manifest.is_permission_allowed() {
            return Err(RegistryError::PermissionDenied(format!(
                "untrusted plugin {} requests control permissions",
                manifest.name
            )));
        }

        let mut entries = self
            .entries
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        if entries.contains_key(&manifest.name) {
            return Err(RegistryError::DuplicateName(manifest.name));
        }

        let name = manifest.name.clone();
        entries.insert(name, PluginEntry::new(manifest));
        Ok(())
    }

    /// 状态转换。
    pub fn transition(
        &self,
        name: &str,
        next: PluginState,
        reason: Option<String>,
    ) -> Result<(), RegistryError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound(name.to_string()))?;

        if !entry.state.can_transition_to(next) {
            return Err(RegistryError::InvalidTransition {
                from: entry.state,
                to: next,
            });
        }

        entry.state = next;
        entry.last_reason = reason;

        if next == PluginState::Failed {
            entry.failure_count += 1;
        }
        if next == PluginState::Active {
            entry.failure_count = 0;
        }

        Ok(())
    }

    /// 查询单个插件。
    pub fn get(&self, name: &str) -> Result<Option<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries.get(name).cloned())
    }

    /// 列出所有插件。
    pub fn list(&self) -> Result<Vec<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<PluginEntry> = entries.values().cloned().collect();
        out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(out)
    }

    /// 按 kind 筛选。
    pub fn list_by_kind(&self, kind: PluginKind) -> Result<Vec<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries
            .values()
            .filter(|e| e.manifest.kind == kind)
            .cloned()
            .collect())
    }

    /// 按 trust level 筛选。
    pub fn list_by_trust(&self, trust: TrustLevel) -> Result<Vec<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries
            .values()
            .filter(|e| e.manifest.trust_level == trust)
            .cloned()
            .collect())
    }

    /// 统计。
    pub fn count(&self) -> Result<usize, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries.len())
    }

    /// 当前 Core 的 API 版本。
    pub fn current_api_version(&self) -> ApiVersion {
        ApiVersion::CURRENT
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegistryError {
    #[error("registry lock poisoned")]
    LockPoisoned,
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("duplicate plugin name: {0}")]
    DuplicateName(String),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("invalid transition: {from:?} → {to:?}")]
    InvalidTransition { from: PluginState, to: PluginState },
}
