use std::path::Path;

use crate::errors::AicoreError;

use super::schema::{AicoreConfig, ConfigError};
/// Loads the configuration from the JSON file.
///
/// File does not exist to return `Ok(default)`; resolution failed to return Err.
pub fn load_from_file(path: impl AsRef<Path>) -> Result<AicoreConfig, ConfigError> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(AicoreConfig::default());
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| AicoreError::ConfigInvalid(format!("{}", e)))?;
    let config: AicoreConfig =
        serde_json::from_str(&content).map_err(|e| AicoreError::ConfigInvalid(format!("{}", e)))?;
    config.validate()?;
    Ok(config)
}

/// Saves the configured JSON file.
pub fn save_to_file(config: &AicoreConfig, path: impl AsRef<Path>) -> Result<(), ConfigError> {
    config.validate()?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AicoreError::ConfigInvalid(format!("create dir: {}", e)))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| AicoreError::ConfigInvalid(format!("serialize: {}", e)))?;
    std::fs::write(path, json).map_err(|e| AicoreError::ConfigInvalid(format!("write: {}", e)))?;
    Ok(())
}

/// Overwrites the configuration from the environment variable.
///
/// Support:
/// - `AICORE_LOG_LEVEL`
/// - `AICORE_LOG_JSON` ("1" / "true")
/// - `AICORE_PLUGINS_DIR`
/// - `AICORE_MEMORY_PATH`
pub fn apply_env_overrides(config: &mut AicoreConfig) {
    if let Ok(v) = std::env::var("AICORE_LOG_LEVEL") {
        config.logging_level = v;
    }
    if let Ok(v) = std::env::var("AICORE_LOG_JSON") {
        config.logging_json = v == "1" || v.eq_ignore_ascii_case("true");
    }
    if let Ok(v) = std::env::var("AICORE_PLUGINS_DIR") {
        config.plugins_dir = Some(v);
    }
    if let Ok(v) = std::env::var("AICORE_MEMORY_PATH") {
        config.memory_path = Some(v);
    }
}

/// Default configuration path.
pub fn default_config_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("config.json")
}
