use serde::{Deserialize, Serialize};

use super::{
    api_version::{parse_api_version, ApiVersion},
    permission::PluginPermission,
};

/// 插件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// 平台适配器。
    Adapter,
    /// AI Provider。
    AiProvider,
    /// 遥测源。
    Telemetry,
    /// 通用扩展。
    Extension,
}

/// 插件信任等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// 随产品发布的插件。
    BuiltIn,
    /// 已签名 / 已验证的插件。
    Trusted,
    /// 本地开发插件。
    LocalDev,
    /// 未验证插件（默认拒绝控制类权限）。
    #[default]
    Untrusted,
}

/// 插件 manifest。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// Plugin API 版本，例如 "1.0"。
    pub api_version: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub trust_level: TrustLevel,
    /// 插件声明实现哪些接口，例如 ["IGpuController"]。
    pub interfaces: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// 人类可读描述。
    #[serde(default)]
    pub description: String,
}

impl PluginManifest {
    /// 校验 manifest 合法性。
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

    /// 是否请求了控制类权限。
    pub fn requests_control(&self) -> bool {
        self.permissions.iter().any(|p| p.is_control())
    }

    /// 未验证插件请求控制类权限 → 拒绝。
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
