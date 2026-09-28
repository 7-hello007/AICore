//! Configure version migration.
//!
//! Semantics:
//! - It's the latest version back in the same way.
//! - `1.x` version completed with the latest `1.0`.
//! - Unknown main version → Err.

use crate::errors::AicoreError;

use super::schema::{AicoreConfig, CONFIG_SCHEMA_VERSION};

/// Moves the configuration to the current schema version.
pub fn migrate_to_current(config: AicoreConfig) -> Result<AicoreConfig, AicoreError> {
    if config.version == CONFIG_SCHEMA_VERSION {
        return Ok(config);
    }
    if config.version.major == CONFIG_SCHEMA_VERSION.major {
        // 1.x → 1.0 No field changes
        let mut migrated = config;
        migrated.version = CONFIG_SCHEMA_VERSION;
        return Ok(migrated);
    }
    Err(AicoreError::ConfigInvalid(format!(
        "cannot migrate config version {} to {}",
        config.version, CONFIG_SCHEMA_VERSION
    )))
}
