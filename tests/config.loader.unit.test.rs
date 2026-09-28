//! Configure loading and migration tests.

use std::path::PathBuf;

use aicore_contracts::config::{
    apply_env_overrides, load_from_file, migrate_to_current, save_to_file, AicoreConfig,
    CONFIG_SCHEMA_VERSION,
};
use aicore_contracts::errors::AicoreError;
use aicore_contracts::schema::SchemaVersion;

fn tmp_config(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aicore-config-test-{}-{}",
        std::process::id(),
        name
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("config.json")
}

#[test]
fn default_config_is_valid() {
    let c = AicoreConfig::default();
    c.validate().unwrap();
    assert_eq!(c.version, CONFIG_SCHEMA_VERSION);
    assert_eq!(c.logging_level, "info");
    assert!(!c.logging_json);
}

#[test]
fn missing_file_returns_default() {
    let path = tmp_config("missing");
    let _ = std::fs::remove_file(&path);
    let c = load_from_file(&path).unwrap();
    assert_eq!(c, AicoreConfig::default());
}

#[test]
fn save_and_load_roundtrip() {
    let path = tmp_config("roundtrip");
    let _ = std::fs::remove_file(&path);

    let c = AicoreConfig {
        logging_level: "debug".to_string(),
        logging_json: true,
        ..Default::default()
    };
    save_to_file(&c, &path).unwrap();

    let loaded = load_from_file(&path).unwrap();
    assert_eq!(loaded, c);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn invalid_log_level_rejected() {
    let c = AicoreConfig {
        logging_level: "verbose".to_string(),
        ..Default::default()
    };
    assert!(matches!(c.validate(), Err(AicoreError::ConfigInvalid(_))));
}

#[test]
fn unsupported_version_rejected() {
    let c = AicoreConfig {
        version: SchemaVersion::new(99, 0),
        ..Default::default()
    };
    assert!(matches!(c.validate(), Err(AicoreError::ConfigInvalid(_))));
}

#[test]
fn migrate_from_1x_to_1_0() {
    let c = AicoreConfig {
        version: SchemaVersion::new(1, 5),
        ..Default::default()
    };
    let migrated = migrate_to_current(c).unwrap();
    assert_eq!(migrated.version, CONFIG_SCHEMA_VERSION);
}

#[test]
fn migrate_current_is_idempotent() {
    let c = AicoreConfig::default();
    let m1 = migrate_to_current(c.clone()).unwrap();
    let m2 = migrate_to_current(m1.clone()).unwrap();
    assert_eq!(m1, m2);
}

#[test]
fn migrate_unknown_major_fails() {
    let c = AicoreConfig {
        version: SchemaVersion::new(99, 0),
        ..Default::default()
    };
    assert!(matches!(
        migrate_to_current(c),
        Err(AicoreError::ConfigInvalid(_))
    ));
}

#[test]
fn env_overrides_applied() {
    std::env::set_var("AICORE_LOG_LEVEL", "warn");
    std::env::set_var("AICORE_LOG_JSON", "1");

    let mut c = AicoreConfig::default();
    apply_env_overrides(&mut c);

    assert_eq!(c.logging_level, "warn");
    assert!(c.logging_json);

    std::env::remove_var("AICORE_LOG_LEVEL");
    std::env::remove_var("AICORE_LOG_JSON");
}
