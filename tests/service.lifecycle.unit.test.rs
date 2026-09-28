//! ServiceLifecycle unit test (based on MockServiceController).

use std::sync::Arc;

use aicore_contracts::platform::service::{IServiceController, ServiceConfig, ServiceState};
use aicore_service_host::{MockServiceController, ServiceLifecycle};

fn make_controller() -> Arc<MockServiceController> {
    Arc::new(MockServiceController::new())
}

fn make_lifecycle(c: Arc<MockServiceController>) -> ServiceLifecycle {
    ServiceLifecycle::new(c)
}

fn config() -> ServiceConfig {
    ServiceConfig {
        name: "aicored".to_string(),
        display_name: "AICore Daemon".to_string(),
        description: "test".to_string(),
        binary_path: "C:\\Test\\aicored.exe".to_string(),
        args: vec![],
        auto_start: true,
        restart_on_failure: true,
        restart_delay_secs: 5,
    }
}

#[test]
fn lifecycle_reports_controller_name() {
    let c = make_controller();
    let l = make_lifecycle(c);
    assert_eq!(l.controller_name(), "mock_service_controller");
}

#[test]
fn install_sets_state_to_stopped() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    assert_eq!(c.query_state(), ServiceState::NotInstalled);

    let r = l.install(&config());
    assert!(r.success);
    assert_eq!(c.query_state(), ServiceState::Stopped);
}

#[test]
fn install_is_idempotent() {
    let c = make_controller();
    let l = make_lifecycle(c);
    let _ = l.install(&config());
    let r2 = l.install(&config());
    assert!(r2.success);
}

#[test]
fn install_rejects_empty_binary() {
    let c = make_controller();
    let l = make_lifecycle(c);
    let bad = ServiceConfig {
        binary_path: "".to_string(),
        ..config()
    };
    let r = l.install(&bad);
    assert!(!r.success);
}

#[test]
fn ensure_started_from_stopped_runs() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    l.install(&config());

    let r = l.ensure_started();
    assert!(r.success);
    assert_eq!(c.query_state(), ServiceState::Running);
}

#[test]
fn ensure_started_when_already_running_is_noop() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    l.install(&config());
    l.ensure_started();

    let r = l.ensure_started();
    assert!(r.success);
    assert_eq!(c.query_state(), ServiceState::Running);
}

#[test]
fn ensure_stopped_from_running_stops() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    l.install(&config());
    l.ensure_started();

    let r = l.ensure_stopped();
    assert!(r.success);
    assert_eq!(c.query_state(), ServiceState::Stopped);
}

#[test]
fn ensure_stopped_when_already_stopped_is_noop() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    l.install(&config());
    let r = l.ensure_stopped();
    assert!(r.success);
}

#[test]
fn uninstall_returns_to_not_installed() {
    let c = make_controller();
    let l = make_lifecycle(c.clone());
    l.install(&config());
    l.ensure_started();

    let r = l.uninstall();
    assert!(r.success);
    assert_eq!(c.query_state(), ServiceState::NotInstalled);
}

#[test]
fn uninstall_on_not_installed_is_noop() {
    let c = make_controller();
    let l = make_lifecycle(c);
    let r = l.uninstall();
    assert!(r.success);
}

#[test]
fn ensure_started_fails_when_not_installed() {
    let c = make_controller();
    let l = make_lifecycle(c);
    let r = l.ensure_started();
    assert!(!r.success);
    assert!(r.message.as_ref().unwrap().contains("not installed"));
}

#[test]
fn ensure_started_when_start_fails_reports_failure() {
    let c = Arc::new(MockServiceController::new().with_failing_start());
    let l = make_lifecycle(c.clone());
    l.install(&config());

    let r = l.ensure_started();
    assert!(!r.success);
    assert_eq!(c.query_state(), ServiceState::Failed);
}
