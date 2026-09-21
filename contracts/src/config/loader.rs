use std::path::Path;

use crate::errors::AicoreError;

use super::schema::{AicoreConfig, ConfigError};
/// 从 JSON 文件加载配置。
///
/// 文件不存在返回 `Ok(default)`；解析失败返回 Err。
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

/// 保存配置到 JSON 文件。
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

/// 从环境变量覆盖配置。
///
/// 支持：
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

/// 默认配置路径。
pub fn default_config_path() -> std::path::PathBuf {
    std::env::temp_dir().join("aicore").join("config.json")
}
