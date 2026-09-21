//! Upgrade + rollback 集成测试。

use std::sync::Arc;

use aicore_contracts::platform::service::{IServiceController, ServiceConfig, ServiceState};
use aicore_service_host::{MockServiceController, ServiceLifecycle, UpgradeOutcome, UpgradePlan};

fn config() -> ServiceConfig {
    ServiceConfig {
        name: "aicored".to_string(),
        display_name: "AICore Daemon".to_string(),
        description: "test".to_string(),
        binary_path: "/test/aicored".to_string(),
        args: vec![],
        auto_start: true,
        restart_on_failure: true,
        restart_delay_secs: 5,
    }
}

fn setup() -> (Arc<MockServiceController>, ServiceLifecycle) {
    let c = Arc::new(MockServiceController::new());
    let l = ServiceLifecycle::new(c.clone());
    l.install(&config());
    l.ensure_started();
    (c, l)
}

#[test]
fn upgrade_success_leaves_running() {
    let (c, _l) = setup();
    let plan = UpgradePlan::new(c.clone());
    let outcome = plan.run("/new/aicored-v2");

    assert_eq!(outcome, UpgradeOutcome::Succeeded);
    assert_eq!(c.query_state(), ServiceState::Running);
    assert_eq!(c.current_binary(), "/new/aicored-v2");
    assert_eq!(c.backup_count(), 1);
}

#[test]
fn upgrade_failure_does_not_change_binary() {
    let c = Arc::new(MockServiceController::new().with_failing_upgrade());
    let l = ServiceLifecycle::new(c.clone());
    l.install(&config());
    l.ensure_started();

    let plan = UpgradePlan::new(c.clone());
    let outcome = plan.run("/new/aicored-v2");

    match outcome {
        UpgradeOutcome::Failed { reason } => {
            assert!(reason.contains("upgrade failed"));
        },
        other => panic!("expected Failed, got {:?}", other),
    }
    // 二进制未改变
    assert_eq!(c.current_binary(), "/mock/bin/aicored");
    assert_eq!(c.backup_count(), 0);
}

#[test]
fn rollback_without_backup_fails() {
    let c = Arc::new(MockServiceController::new());
    let l = ServiceLifecycle::new(c.clone());
    l.install(&config());

    let r = c.rollback();
    assert!(!r.success);
    assert!(r.message.as_ref().unwrap().contains("no backup"));
}

#[test]
fn upgrade_preserves_previous_binary_in_backup() {
    let (c, _l) = setup();
    let original = c.current_binary();

    let plan = UpgradePlan::new(c.clone());
    let outcome = plan.run("/new/aicored-v2");
    assert_eq!(outcome, UpgradeOutcome::Succeeded);

    // rollback 恢复原始
    c.rollback().unwrap_ok();
    assert_eq!(c.current_binary(), original);
}

// 便于断言的便捷扩展
trait UnwrapOk {
    fn unwrap_ok(self);
}
impl UnwrapOk for aicore_contracts::platform::service::ServiceOperationResult {
    fn unwrap_ok(self) {
        assert!(self.success, "expected success: {:?}", self.message);
    }
}
