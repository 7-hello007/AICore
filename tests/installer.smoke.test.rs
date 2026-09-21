//! Installer 静态冒烟测试。
//!
//! 不启动真实服务，只验证 deploy/ 结构与关键内容。

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {}", p.display(), e))
}

#[test]
fn deploy_dir_exists() {
    let root = repo_root();
    assert!(root.join("deploy").is_dir());
    assert!(root.join("deploy/linux").is_dir());
    assert!(root.join("deploy/packaging").is_dir());
    assert!(root.join("deploy/windows").is_dir());
}

#[test]
fn systemd_unit_has_required_sections() {
    let content = read("deploy/linux/aicored.service");
    assert!(content.contains("[Unit]"));
    assert!(content.contains("[Service]"));
    assert!(content.contains("[Install]"));
    assert!(content.contains("ExecStart=/usr/local/bin/aicored"));
    assert!(content.contains("User=aicore"));
    assert!(content.contains("Restart=on-failure"));
    assert!(content.contains("WantedBy=multi-user.target"));
}

#[test]
fn systemd_unit_has_security_hardening() {
    let content = read("deploy/linux/aicored.service");
    assert!(content.contains("NoNewPrivileges=true"));
    assert!(content.contains("ProtectSystem=strict"));
    assert!(content.contains("PrivateTmp=true"));
    assert!(content.contains("ReadWritePaths=/var/lib/aicore"));
}

#[test]
fn install_script_creates_user_and_enables_service() {
    let content = read("deploy/linux/install.sh");
    assert!(content.contains("useradd"));
    assert!(content.contains("groupadd"));
    assert!(content.contains("systemctl enable"));
    assert!(content.contains("systemctl restart"));
}

#[test]
fn uninstall_supports_purge() {
    let content = read("deploy/linux/uninstall.sh");
    assert!(content.contains("--purge"));
    assert!(content.contains("userdel"));
    assert!(content.contains("groupdel"));
}

#[test]
fn upgrade_script_has_rollback_path() {
    let content = read("deploy/linux/upgrade.sh");
    assert!(content.contains("BACKUP"));
    assert!(content.contains("rolling back"));
    assert!(content.contains("health"));
}

#[test]
fn rollback_script_lists_backups() {
    let content = read("deploy/linux/rollback.sh");
    assert!(content.contains("available backups"));
    assert!(content.contains("BACKUP_ROOT"));
}

#[test]
fn deb_control_template_has_placeholders() {
    let content = read("deploy/packaging/debian/control.template");
    assert!(content.contains("@VERSION@"));
    assert!(content.contains("@ARCH@"));
    assert!(content.contains("Package: aicore"));
}

#[test]
fn deb_scripts_have_shebang() {
    for f in [
        "deploy/packaging/debian/postinst",
        "deploy/packaging/debian/prerm",
        "deploy/packaging/debian/postrm",
    ] {
        let content = read(f);
        assert!(content.starts_with("#!"), "{} should start with #!", f);
        assert!(content.contains("set -e"), "{} should have set -e", f);
    }
}

#[test]
fn build_deb_checks_prerequisites() {
    let content = read("deploy/packaging/build_deb.sh");
    assert!(content.contains("dpkg-deb"));
    assert!(content.contains("cargo build --release"));
}

#[test]
fn windows_scripts_have_powershell_header() {
    let content = read("deploy/windows/install.ps1");
    assert!(content.contains("param("));
    assert!(content.contains("$InstallDir"));
}
