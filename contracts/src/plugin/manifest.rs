use serde::{Deserialize, Serialize};

use super::{
    api_version::{parse_api_version, ApiVersion},
    permission::PluginPermission,
};

/// Plugin type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Platform adapter.
    Adapter,
    /// AI Provider。
    AiProvider,
    /// Telemetry source.
    Telemetry,
    /// Universal expansion.
    Extension,
}

/// Plugin trust level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Plugin with product release.
    BuiltIn,
    /// Signed/ validated plugins.
    Trusted,
    /// Local development plugin.
    LocalDev,
    /// No Plugin Validation (Default to Control Class Permissions).
    #[default]
    Untrusted,
}

/// plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// Plugin API version, for example "1.0".
    pub api_version: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub trust_level: TrustLevel,
    /// The plugin states which interfaces are implemented, such as ["IGpuController"].
    pub interfaces: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// Humans can read descriptions.
    #[serde(default)]
    pub description: String,
}

impl PluginManifest {
    /// Validates the legal validity of a person.
    pub fn validate(&self) -> Result<ApiVersion, crate::errors::AicoreError> {
        use crate::errors::AicoreError;

        if self.name.trim().is_empty() {
            return Err(AicoreError::PluginInvalid(
                "plugin name is empty".to_string(),
            ));
        }
        if self.name.len() > 128 {
            return Err(AicoreError::PluginInvalid(
                "plugin name exceeds 128 characters".to_string(),
            ));
        }
        if self.version.trim().is_empty() {
            return Err(AicoreError::PluginInvalid(
                "plugin version is empty".to_string(),
            ));
        }
        let api = parse_api_version(&self.api_version)
            .ok_or_else(|| AicoreError::PluginInvalid("invalid api_version format".to_string()))?;
        if !api.is_compatible_with(ApiVersion::CURRENT) {
            return Err(AicoreError::PluginApiVersionMismatch {
                plugin: api,
                core: ApiVersion::CURRENT,
            });
        }
        if self.interfaces.is_empty() {
            return Err(AicoreError::PluginInvalid(
                "plugin declares no interfaces".to_string(),
            ));
        }
        Ok(api)
    }

    /// Whether control type privileges are requested.
    pub fn requests_control(&self) -> bool {
        self.permissions.iter().any(|p| p.is_control())
    }

    /// Unvalidated plugin requests control class permissions.
    pub fn is_permission_allowed(&self) -> bool {
        if self.trust_level == TrustLevel::Untrusted && self.requests_control() {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ManifestError {
    #[error("plugin name is empty")]
    EmptyName,
    #[error("plugin name exceeds 128 characters")]
    NameTooLong,
    #[error("plugin version is empty")]
    EmptyVersion,
    #[error("invalid api_version format")]
    InvalidApiVersion,
    #[error("api_version {plugin} incompatible with core {core}")]
    ApiVersionMismatch {
        plugin: ApiVersion,
        core: ApiVersion,
    },
    #[error("plugin declares no interfaces")]
    NoInterfaces,
}
