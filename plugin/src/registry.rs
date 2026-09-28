use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::plugin::{
    api_version::ApiVersion,
    manifest::{PluginKind, PluginManifest, TrustLevel},
    state::PluginState,
};
use serde::{Deserialize, Serialize};

/// One of the plugins in Registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub state: PluginState,
    /// Last change of status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reason: Option<String>,
    /// Number of consecutive failures.
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

/// Plugin registration form.
///
/// Line is secure. All state changes are completed by `transition`, mandatory status machine rules.
pub struct PluginRegistry {
    entries: RwLock<HashMap<String, PluginEntry>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new plugin (initial state)
    ///
    /// Refuses to overwrite activated plugins if the name already exists.
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

    /// Status conversion.
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

    /// Commit a verified replacement while the old process remains active.
    /// Failed staging never changes the registry's visible version.
    pub fn replace_active_manifest(&self, manifest: PluginManifest) -> Result<(), RegistryError> {
        self.replace_active_manifest_after(manifest, || Ok(()))
    }

    /// Keep the registry version unchanged until the old process has stopped.
    /// Validation and the stop callback run under the same write lock so a
    /// registry failure cannot occur after a successful stop.
    pub fn replace_active_manifest_after(
        &self,
        manifest: PluginManifest,
        stop_old: impl FnOnce() -> Result<(), RegistryError>,
    ) -> Result<(), RegistryError> {
        manifest
            .validate()
            .map_err(|error| RegistryError::InvalidManifest(error.to_string()))?;
        if !manifest.is_permission_allowed() {
            return Err(RegistryError::PermissionDenied(manifest.name));
        }
        let mut entries = self
            .entries
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let entry = entries
            .get_mut(&manifest.name)
            .ok_or_else(|| RegistryError::NotFound(manifest.name.clone()))?;
        if entry.state != PluginState::Active {
            return Err(RegistryError::InvalidTransition {
                from: entry.state,
                to: PluginState::Active,
            });
        }
        stop_old()?;
        entry.manifest = manifest;
        entry.last_reason = None;
        Ok(())
    }

    /// Query individual plugins.
    pub fn get(&self, name: &str) -> Result<Option<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries.get(name).cloned())
    }

    /// Lists all the plugins.
    pub fn list(&self) -> Result<Vec<PluginEntry>, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<PluginEntry> = entries.values().cloned().collect();
        out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(out)
    }

    /// Press kind to filter.
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

    /// Filter by trust level.
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

    /// Statistics.
    pub fn count(&self) -> Result<usize, RegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(entries.len())
    }

    /// Current Core version of API.
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
    #[error("old plugin stop failed: {0}")]
    ProcessStopFailed(String),
}
