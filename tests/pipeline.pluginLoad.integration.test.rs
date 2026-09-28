//! Plugin found + registration integration test (real file system).

use std::path::{Path, PathBuf};

use aicore_contracts::plugin::{
    manifest::{PluginKind, PluginManifest, TrustLevel},
    permission::PluginPermission,
    state::PluginState,
};
use aicore_plugin::{discover_from_dir, register_discovered, PluginRegistry};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aicore-plugin-test-{}-{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn write_manifest(dir: &Path, file: &str, manifest: &PluginManifest) {
    let path = dir.join(file);
    std::fs::write(path, serde_json::to_string_pretty(manifest).unwrap()).unwrap();
}

fn good_manifest(name: &str) -> PluginManifest {
    PluginManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        api_version: "1.0".to_string(),
        kind: PluginKind::Adapter,
        trust_level: TrustLevel::Trusted,
        interfaces: vec!["IProcessController".to_string()],
        permissions: vec![PluginPermission::TelemetryRead],
        description: "integration test".to_string(),
    }
}

#[test]
fn discover_from_empty_dir_returns_empty() {
    let dir = tmp_dir("empty");
    let _ = std::fs::remove_dir_all(&dir);
    let report = discover_from_dir(&dir);
    assert!(report.discovered.is_empty());
    assert!(report.rejected.is_empty());
}

#[test]
fn discover_and_register_plugin() {
    let dir = tmp_dir("one");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    write_manifest(&dir, "test.plugin.json", &good_manifest("discovered-1"));

    let report = discover_from_dir(&dir);
    assert_eq!(report.discovered.len(), 1);
    assert_eq!(report.discovered[0].manifest.name, "discovered-1");

    let reg = PluginRegistry::new();
    let n = register_discovered(&reg, &report).unwrap();
    assert_eq!(n, 1);
    assert_eq!(reg.count().unwrap(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_manifest_is_rejected_but_doesnt_block_others() {
    let dir = tmp_dir("mixed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Legal
    write_manifest(&dir, "good.plugin.json", &good_manifest("good"));
    // Illegal JSON
    std::fs::write(dir.join("bad.plugin.json"), "{ not valid json").unwrap();
    // Illegal API version
    let mut bad2 = good_manifest("bad2");
    bad2.api_version = "99.0".to_string();
    write_manifest(&dir, "bad2.plugin.json", &bad2);

    let report = discover_from_dir(&dir);
    assert_eq!(report.discovered.len(), 1, "only good should be discovered");
    assert_eq!(report.rejected.len(), 2);

    let reg = PluginRegistry::new();
    let n = register_discovered(&reg, &report).unwrap();
    assert_eq!(n, 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untrusted_plugin_with_control_permission_is_rejected_at_register() {
    let dir = tmp_dir("untrusted");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut m = good_manifest("untrusted-1");
    m.trust_level = TrustLevel::Untrusted;
    m.permissions.push(PluginPermission::ProcessControl);
    write_manifest(&dir, "untrusted.plugin.json", &m);

    let report = discover_from_dir(&dir);
    // The discovery phase inspects manifest structure without granting permissions
    assert_eq!(report.discovered.len(), 1);

    // Register Phase Rejected
    let reg = PluginRegistry::new();
    let n = register_discovered(&reg, &report).unwrap();
    assert_eq!(n, 0);
    assert_eq!(reg.count().unwrap(), 0);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn registered_plugin_ends_in_discovered_state() {
    let dir = tmp_dir("state");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    write_manifest(&dir, "p.plugin.json", &good_manifest("state-check"));

    let report = discover_from_dir(&dir);
    let reg = PluginRegistry::new();
    register_discovered(&reg, &report).unwrap();

    let entry = reg.get("state-check").unwrap().unwrap();
    assert_eq!(entry.state, PluginState::Discovered);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_plugin_json_files_are_ignored() {
    let dir = tmp_dir("ignore");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("config.json"), "{}").unwrap();
    std::fs::write(dir.join("readme.txt"), "hi").unwrap();
    write_manifest(&dir, "only.plugin.json", &good_manifest("only"));

    let report = discover_from_dir(&dir);
    assert_eq!(report.discovered.len(), 1);
    assert_eq!(report.discovered[0].manifest.name, "only");

    let _ = std::fs::remove_dir_all(&dir);
}
