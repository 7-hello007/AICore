//! 进程隔离测试：spawn / ping / execute / shutdown / crash。

use std::path::PathBuf;

use aicore_contracts::plugin::host::{IPluginHost, PluginHostState, PluginProcessSpec};
use aicore_plugin::{HostRequest, HostResponse, SubprocessPluginHost};

fn echo_path() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap();
    let target_dir = root.join("target").join("debug");

    #[cfg(target_os = "linux")]
    let name = "plugin-echo";
    #[cfg(target_os = "windows")]
    let name = "plugin-echo.exe";
    #[cfg(target_os = "macos")]
    let name = "plugin-echo";

    let p = target_dir.join(name);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn make_host() -> SubprocessPluginHost {
    SubprocessPluginHost::new()
}

fn make_spec(path: &std::path::Path) -> PluginProcessSpec {
    PluginProcessSpec {
        plugin_id: "test.echo".to_string(),
        artifact_path: path.to_string_lossy().to_string(),
        timeout_ms: 2000,
        memory_limit_mb: 64,
    }
}

#[test]
fn host_reports_name() {
    let h = make_host();
    assert_eq!(h.host_name(), "subprocess_plugin_host");
}

#[test]
fn spawn_missing_binary_fails() {
    let h = make_host();
    let spec = PluginProcessSpec {
        plugin_id: "missing".to_string(),
        artifact_path: "/nonexistent/binary".to_string(),
        timeout_ms: 1000,
        memory_limit_mb: 16,
    };
    assert!(h.spawn(&spec).is_err());
}

#[test]
fn spawn_ping_shutdown() {
    let path = match echo_path() {
        Some(p) => p,
        None => {
            eprintln!("skip: plugin-echo not built; run `cargo build -p plugin-echo`");
            return;
        },
    };

    let h = make_host();
    let handle = h.spawn(&make_spec(&path)).unwrap();
    assert_eq!(h.state(&handle), PluginHostState::Running);

    let resp = h.call(&handle, &HostRequest::Ping).unwrap();
    assert!(matches!(resp, HostResponse::Pong));

    let resp = h.call(&handle, &HostRequest::Execute { input: 5 }).unwrap();
    match resp {
        HostResponse::Result { output } => assert_eq!(output, 5 * 2 + 1),
        other => panic!("unexpected: {:?}", other),
    }

    h.stop(&handle).unwrap();
    assert_eq!(h.state(&handle), PluginHostState::Stopped);
}

#[test]
fn crash_is_recorded_and_core_survives() {
    let path = match echo_path() {
        Some(p) => p,
        None => return,
    };

    let h = make_host();
    let handle = h.spawn(&make_spec(&path)).unwrap();

    // 通过 Shutdown 让进程自然退出，模拟"退出"
    let _ = h.call(&handle, &HostRequest::Shutdown);

    // 稍等进程退出
    std::thread::sleep(std::time::Duration::from_millis(200));

    // 再次调用触发 EOF / EPIPE 检测
    let err = h.call(&handle, &HostRequest::Ping).unwrap_err();
    let err_text = err.to_string();

    assert!(
        err_text.contains("exited")
            || err_text.contains("read failed")
            || err_text.contains("write failed"),
        "unexpected: {}",
        err_text
    );

    let crashes = h.recent_crashes();
    assert!(!crashes.is_empty(), "expected crash event");
    assert_eq!(crashes[0].plugin_id, "test.echo");

    h.clear_crashes();
    assert!(h.recent_crashes().is_empty());
}

#[test]
fn multiple_plugins_can_coexist() {
    let path = match echo_path() {
        Some(p) => p,
        None => return,
    };

    let h = make_host();
    let spec1 = PluginProcessSpec {
        plugin_id: "a".to_string(),
        ..make_spec(&path)
    };
    let spec2 = PluginProcessSpec {
        plugin_id: "b".to_string(),
        ..make_spec(&path)
    };

    let h1 = h.spawn(&spec1).unwrap();
    let h2 = h.spawn(&spec2).unwrap();

    assert_ne!(h1.pid, h2.pid);
    assert_eq!(h.state(&h1), PluginHostState::Running);
    assert_eq!(h.state(&h2), PluginHostState::Running);

    let _ = h.call(&h1, &HostRequest::Ping).unwrap();
    let _ = h.call(&h2, &HostRequest::Ping).unwrap();

    h.stop(&h1).unwrap();
    assert_eq!(h.state(&h1), PluginHostState::Stopped);
    assert_eq!(h.state(&h2), PluginHostState::Running);

    h.stop(&h2).unwrap();
}

#[test]
fn unknown_handle_returns_stopped() {
    use aicore_contracts::plugin::host::PluginProcessHandle;

    let h = make_host();
    let fake = PluginProcessHandle {
        handle_id: "unknown".to_string(),
        plugin_id: "x".to_string(),
        pid: 0,
        started_at_unix_ms: 0,
    };
    assert_eq!(h.state(&fake), PluginHostState::Stopped);
    assert!(h.call(&fake, &HostRequest::Ping).is_err());
    assert!(h.stop(&fake).is_err());
}
