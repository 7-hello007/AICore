//! 插件生命周期与隔离测试。

use aicore_contracts::plugin::{
    manifest::{PluginKind, PluginManifest, TrustLevel},
    permission::PluginPermission,
    state::PluginState,
};
use aicore_plugin::{PluginIsolation, PluginRegistry, RegistryError};

fn manifest(name: &str) -> PluginManifest {
    PluginManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        api_version: "1.0".to_string(),
        kind: PluginKind::Adapter,
        trust_level: TrustLevel::Trusted,
        interfaces: vec!["ITest".to_string()],
        permissions: vec![PluginPermission::TelemetryRead],
        description: "test".to_string(),
    }
}

#[test]
fn register_new_plugin_is_discovered() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();
    assert_eq!(reg.count().unwrap(), 1);
    let e = reg.get("p1").unwrap().unwrap();
    assert_eq!(e.state, PluginState::Discovered);
}

#[test]
fn duplicate_name_rejected() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();
    assert!(matches!(
        reg.register(manifest("p1")),
        Err(RegistryError::DuplicateName(_))
    ));
}

#[test]
fn invalid_transition_rejected() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();

    // Discovered → Active 非法（必须先 Validating → Ready）
    let err = reg.transition("p1", PluginState::Active, None).unwrap_err();
    assert!(matches!(err, RegistryError::InvalidTransition { .. }));
}

#[test]
fn full_lifecycle_transitions() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();

    reg.transition("p1", PluginState::Validating, None).unwrap();
    reg.transition("p1", PluginState::Ready, None).unwrap();
    reg.transition("p1", PluginState::Active, None).unwrap();
    assert!(reg.get("p1").unwrap().unwrap().state.is_operational());

    reg.transition("p1", PluginState::Disabled, None).unwrap();
    reg.transition("p1", PluginState::Active, None).unwrap();
    assert!(reg.get("p1").unwrap().unwrap().state.is_operational());
}

#[test]
fn rejection_terminal() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();
    reg.transition("p1", PluginState::Validating, None).unwrap();
    reg.transition("p1", PluginState::Rejected, Some("bad".to_string()))
        .unwrap();

    // Rejected 是终态
    assert!(reg.get("p1").unwrap().unwrap().state.is_terminal());
    assert!(reg.transition("p1", PluginState::Active, None).is_err());
}

#[test]
fn failure_increments_count_and_can_recover() {
    let reg = PluginRegistry::new();
    reg.register(manifest("p1")).unwrap();
    reg.transition("p1", PluginState::Validating, None).unwrap();
    reg.transition("p1", PluginState::Ready, None).unwrap();
    reg.transition("p1", PluginState::Active, None).unwrap();

    reg.transition("p1", PluginState::Failed, Some("crash".to_string()))
        .unwrap();
    assert_eq!(reg.get("p1").unwrap().unwrap().failure_count, 1);

    // Failed → Disabled → Active 恢复
    reg.transition("p1", PluginState::Disabled, None).unwrap();
    reg.transition("p1", PluginState::Active, None).unwrap();
    assert_eq!(reg.get("p1").unwrap().unwrap().failure_count, 0);
}

#[test]
fn list_by_kind_filters() {
    let reg = PluginRegistry::new();
    let mut m1 = manifest("adapter-1");
    m1.kind = PluginKind::Adapter;
    let mut m2 = manifest("ai-1");
    m2.kind = PluginKind::AiProvider;
    reg.register(m1).unwrap();
    reg.register(m2).unwrap();

    let adapters = reg.list_by_kind(PluginKind::Adapter).unwrap();
    assert_eq!(adapters.len(), 1);
    assert_eq!(adapters[0].manifest.name, "adapter-1");
}

#[test]
fn untrusted_control_rejected_at_register() {
    let reg = PluginRegistry::new();
    let mut m = manifest("bad");
    m.trust_level = TrustLevel::Untrusted;
    m.permissions.push(PluginPermission::ProcessControl);
    assert!(matches!(
        reg.register(m),
        Err(RegistryError::PermissionDenied(_))
    ));
}

#[test]
fn isolation_triggers_after_threshold() {
    let iso = PluginIsolation::new("p1", 3);
    assert!(!iso.record_failure());
    assert!(!iso.record_failure());
    assert!(iso.record_failure()); // 第 3 次触发
    assert_eq!(iso.failure_count(), 3);
}

#[test]
fn isolation_success_resets() {
    let iso = PluginIsolation::new("p1", 3);
    iso.record_failure();
    iso.record_failure();
    iso.record_success();
    assert_eq!(iso.failure_count(), 0);
}

#[test]
fn recommended_state_after_threshold() {
    let iso = PluginIsolation::new("p1", 2);
    iso.record_failure();
    iso.record_failure();
    assert_eq!(
        iso.recommended_state(PluginState::Active),
        PluginState::Failed
    );
}
