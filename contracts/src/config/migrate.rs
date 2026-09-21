//! 配置版本迁移。
//!
//! 语义：
//! - 已是最新版本 → 原样返回。
//! - `1.x` 版本 → 补全为最新 `1.0`。
//! - 未知主版本 → Err。

use crate::errors::AicoreError;

use super::schema::{AicoreConfig, CONFIG_SCHEMA_VERSION};

/// 将配置迁移到当前 schema 版本。
pub fn migrate_to_current(config: AicoreConfig) -> Result<AicoreConfig, AicoreError> {
    if config.version == CONFIG_SCHEMA_VERSION {
        return Ok(config);
    }
    if config.version.major == CONFIG_SCHEMA_VERSION.major {
        // 1.x → 1.0 无字段变更
        let mut migrated = config;
        migrated.version = CONFIG_SCHEMA_VERSION;
        return Ok(migrated);
    }
    Err(AicoreError::ConfigInvalid(format!(
        "cannot migrate config version {} to {}",
        config.version, CONFIG_SCHEMA_VERSION
    )))
}
