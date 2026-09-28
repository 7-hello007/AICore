use serde::{Deserialize, Serialize};

use crate::errors::AicoreError;
use crate::runtime::daemon_config::DaemonConfig;
use crate::schema::SchemaVersion;

/// Configure the schema version.
///
/// Uncompatible change incremental major; add optional field incremental minor.
pub const CONFIG_SCHEMA_VERSION: SchemaVersion = SchemaVersion::CURRENT;

/// AICore top layer configuration.
///
/// Load priority (high over low):
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

/// Compatible alias: `ConfigError` = `AicoreError`.
///
/// All variants have been merged:
/// - `EmptyVersion` / `UnsupportedVersion` / `InvalidDaemonConfig` /
///   `InvalidLogLevel` / `ParseError` / `MigrationError`
///   → `AicoreError::ConfigInvalid`
///
/// See `docs/decisions/ADR-005-unified-error-type.md` for more details.
pub type ConfigError = crate::errors::AicoreError;
