use serde::{Deserialize, Serialize};

use crate::errors::AicoreError;
use crate::runtime::daemon_config::DaemonConfig;
use crate::schema::SchemaVersion;

/// 配置 schema 版本。
///
/// 不兼容变更递增 major；增加 optional 字段递增 minor。
pub const CONFIG_SCHEMA_VERSION: SchemaVersion = SchemaVersion::CURRENT;

/// AICore 顶层配置。
///
/// 加载优先级（高覆盖低）：
/// 1. Built-in defaults
/// 2. System config file
/// 3. User config file
/// 4. Environment variables
/// 5. CLI args
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AicoreConfig {
    pub version: SchemaVersion,
    pub daemon: DaemonConfig,
    pub plugins_dir: Option<String>,
    pub memory_path: Option<String>,
    pub logging_level: String,
    pub logging_json: bool,
}

impl Default for AicoreConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_SCHEMA_VERSION,
            daemon: DaemonConfig::default(),
            plugins_dir: None,
            memory_path: None,
            logging_level: "info".to_string(),
            logging_json: false,
        }
    }
}

impl AicoreConfig {
    pub fn validate(&self) -> Result<(), AicoreError> {
        if self.version.major != CONFIG_SCHEMA_VERSION.major {
            return Err(AicoreError::ConfigInvalid(format!(
                "unsupported config version: {}",
                self.version
            )));
        }
        self.daemon
            .validate()
            .map_err(|e| AicoreError::ConfigInvalid(format!("daemon config: {}", e)))?;
        if !matches!(
            self.logging_level.as_str(),
            "trace" | "debug" | "info" | "warn" | "error"
        ) {
            return Err(AicoreError::ConfigInvalid(format!(
                "invalid log level: {}",
                self.logging_level
            )));
        }
        Ok(())
    }
}

/// 兼容别名：`ConfigError` = `AicoreError`。
///
/// 所有变体已合并：
/// - `EmptyVersion` / `UnsupportedVersion` / `InvalidDaemonConfig` /
///   `InvalidLogLevel` / `ParseError` / `MigrationError`
///   → `AicoreError::ConfigInvalid`
///
/// 详见 `docs/decisions/ADR-005-unified-error-type.md`。
pub type ConfigError = crate::errors::AicoreError;
