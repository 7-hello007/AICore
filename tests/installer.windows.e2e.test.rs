//! Windows instiller static verification (not physically installed).

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn windows_deploy_dir_exists() {
    let root = repo_root();
    assert!(root.join("deploy/windows").is_dir());
}

#[test]
fn windows_service_scripts_exist() {
    let root = repo_root();
    let base = root.join("deploy/windows");
    for f in [
        "service-install.ps1",
        "service-uninstall.ps1",
        "service-upgrade.ps1",
        "service-rollback.ps1",
    ] {
        assert!(base.join(f).exists(), "missing {}", f);
    }
}

#[test]
fn wix_template_exists() {
    let root = repo_root();
    assert!(root.join("deploy/windows/wix/aicore.wxs").exists());
}

#[test]
fn windows_ci_workflow_exists() {
    let root = repo_root();
    let ci = root.join(".github/workflows/ci.yml");
    let content = std::fs::read_to_string(ci).expect("read ci.yml");
    assert!(content.contains("windows-latest"), "no windows job");
    assert!(
        content.contains("windows-production"),
        "no windows-production job"
    );
}

#[test]
fn windows_smoke_script_exists() {
    let root = repo_root();
    assert!(root.join("deploy/windows/ci/windows-smoke.ps1").exists());
}

#[test]
fn service_host_crate_exists() {
    let root = repo_root();
    assert!(root.join("service-host/Cargo.toml").exists());
    assert!(root.join("service-host/src/lifecycle.rs").exists());
    assert!(root.join("service-host/src/upgrade.rs").exists());
    assert!(root.join("service-host/src/mock.rs").exists());
    assert!(root.join("service-host/src/windows/controller.rs").exists());
}
