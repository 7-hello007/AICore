//! Plugin manifest 校验测试。

use aicore_contracts::errors::AicoreError;
use aicore_contracts::plugin::{
    api_version::{parse_api_version, ApiVersion},
    manifest::{PluginKind, PluginManifest, TrustLevel},
    permission::PluginPermission,
};

fn valid_manifest() -> PluginManifest {
    PluginManifest {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        api_version: "1.0".to_string(),
        kind: PluginKind::Adapter,
        trust_level: TrustLevel::Trusted,
        interfaces: vec!["IProcessController".to_string()],
        permissions: vec![PluginPermission::ProcessRead],
        description: "test".to_string(),
    }
}

#[test]
fn valid_manifest_passes() {
    let m = valid_manifest();
    let api = m.validate().unwrap();
    assert_eq!(api, ApiVersion::CURRENT);
}

#[test]
fn empty_name_rejected() {
    let mut m = valid_manifest();
    m.name = "".to_string();
    match m.validate().unwrap_err() {
        AicoreError::PluginInvalid(msg) => assert!(msg.contains("name is empty")),
        e => panic!("expected PluginInvalid, got {:?}", e),
    }
}

#[test]
fn name_too_long_rejected() {
    let mut m = valid_manifest();
    m.name = "x".repeat(200);
    assert!(matches!(
        m.validate().unwrap_err(),
        AicoreError::PluginInvalid(_)
    ));
}

#[test]
fn invalid_api_version_rejected() {
    let mut m = valid_manifest();
    m.api_version = "abc".to_string();
    assert!(matches!(
        m.validate().unwrap_err(),
        AicoreError::PluginInvalid(_)
    ));
}

#[test]
fn api_version_mismatch_rejected() {
    let mut m = valid_manifest();
    m.api_version = "2.0".to_string();
    match m.validate().unwrap_err() {
        AicoreError::PluginApiVersionMismatch { plugin, core } => {
            assert_eq!(plugin.major, 2);
            assert_eq!(core.major, 1);
        },
        e => panic!("expected mismatch, got {:?}", e),
    }
}

#[test]
fn higher_minor_version_ok() {
    let mut m = valid_manifest();
    m.api_version = "1.5".to_string();
    // 主版本相同，插件次版本 5 > Core 次版本 0 → 应拒绝
    assert!(m.validate().is_err());
}

#[test]
fn empty_interfaces_rejected() {
    let mut m = valid_manifest();
    m.interfaces.clear();
    assert!(matches!(
        m.validate().unwrap_err(),
        AicoreError::PluginInvalid(_)
    ));
}

#[test]
fn untrusted_control_permission_rejected() {
    let mut m = valid_manifest();
    m.trust_level = TrustLevel::Untrusted;
    m.permissions.push(PluginPermission::ProcessControl);
    assert!(!m.is_permission_allowed());
}

#[test]
fn untrusted_readonly_permission_allowed() {
    let mut m = valid_manifest();
    m.trust_level = TrustLevel::Untrusted;
    // 只有 read 权限
    assert!(m.is_permission_allowed());
}

#[test]
fn trusted_control_permission_allowed() {
    let mut m = valid_manifest();
    m.trust_level = TrustLevel::Trusted;
    m.permissions.push(PluginPermission::ProcessControl);
    assert!(m.is_permission_allowed());
}

#[test]
fn parse_api_version_handles_valid_input() {
    assert_eq!(parse_api_version("1.0"), Some(ApiVersion::new(1, 0)));
    assert_eq!(parse_api_version("2.5"), Some(ApiVersion::new(2, 5)));
    assert_eq!(parse_api_version("abc"), None);
    assert_eq!(parse_api_version("1"), None);
    assert_eq!(parse_api_version("1.2.3"), None);
}

#[test]
fn api_version_compatibility() {
    let core = ApiVersion::new(1, 5);
    assert!(ApiVersion::new(1, 0).is_compatible_with(core));
    assert!(ApiVersion::new(1, 5).is_compatible_with(core));
    assert!(!ApiVersion::new(1, 6).is_compatible_with(core));
    assert!(!ApiVersion::new(2, 0).is_compatible_with(core));
}

#[test]
fn control_permission_detection() {
    assert!(PluginPermission::ProcessControl.is_control());
    assert!(PluginPermission::PowerControl.is_control());
    assert!(PluginPermission::GpuControl.is_control());
    assert!(PluginPermission::NpuControl.is_control());
    assert!(PluginPermission::AdapterRegistration.is_control());
    assert!(!PluginPermission::TelemetryRead.is_control());
    assert!(!PluginPermission::ProcessRead.is_control());
    assert!(!PluginPermission::AiProvider.is_control());
}
