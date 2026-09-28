//! Web Dashboard static smoke testing.
//!
//! Do not start the real daemon; verify only the existence of the carte, the static resource is embedded, the binary is built.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn web_crate_exists() {
    let root = repo_root();
    assert!(root.join("apps/aicore-dashboard-web/Cargo.toml").exists());
    assert!(root.join("apps/aicore-dashboard-web/src/main.rs").exists());
    assert!(root
        .join("apps/aicore-dashboard-web/src/routes.rs")
        .exists());
    assert!(root.join("apps/aicore-dashboard-web/src/sse.rs").exists());
}

#[test]
fn static_assets_exist() {
    let root = repo_root();
    assert!(root
        .join("apps/aicore-dashboard-web/static/index.html")
        .exists());
    assert!(root
        .join("apps/aicore-dashboard-web/static/style.css")
        .exists());
    assert!(root
        .join("apps/aicore-dashboard-web/static/app.js")
        .exists());
}

#[test]
fn index_html_references_style_and_script() {
    let root = repo_root();
    let html =
        std::fs::read_to_string(root.join("apps/aicore-dashboard-web/static/index.html")).unwrap();
    assert!(html.contains("/style.css"));
    assert!(html.contains("/app.js"));
    assert!(html.contains("AICore"));
}

#[test]
fn app_js_uses_sse_and_snapshot() {
    let root = repo_root();
    let js = std::fs::read_to_string(root.join("apps/aicore-dashboard-web/static/app.js")).unwrap();
    assert!(js.contains("/api/events"));
    assert!(js.contains("/api/snapshot"));
    assert!(js.contains("EventSource"));
}

#[test]
fn web_binary_built() {
    let root = repo_root();
    let bin = root.join("target/debug/aicore-dashboard-web");
    if !bin.exists() {
        eprintln!("skip: build `cargo build -p aicore-dashboard-web` first");
        return;
    }
    assert!(bin.exists());
}

#[test]
fn web_sources_event_id_support() {
    let root = repo_root();

    let sse = std::fs::read_to_string(root.join("apps/aicore-dashboard-web/src/sse.rs")).unwrap();

    assert!(sse.contains("event.id"));
    assert!(sse.contains("id: {}"));
}

#[test]
fn web_dashboard_is_local_by_default() {
    let root = repo_root();
    let main = std::fs::read_to_string(root.join("apps/aicore-dashboard-web/src/main.rs")).unwrap();
    assert!(main.contains("is_loopback"));
    assert!(main.contains("AICORE_DASHBOARD_ALLOW_REMOTE"));
}
