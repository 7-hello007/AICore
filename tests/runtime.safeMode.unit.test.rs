use aicore_contracts::runtime::{config::AdaptiveLoopConfig, mode::RuntimeMode};
use aicore_runtime::SafeModeController;

fn config() -> AdaptiveLoopConfig {
    AdaptiveLoopConfig {
        degraded_threshold: 2,
        safe_threshold: 4,
        recovery_threshold: 2,
    }
}

#[test]
fn starts_in_normal() {
    let c = SafeModeController::new(config());
    assert_eq!(c.current_mode(), RuntimeMode::Normal);
    assert_eq!(c.consecutive_failures(), 0);
    assert_eq!(c.consecutive_successes(), 0);
}

#[test]
fn single_failure_stays_normal() {
    let c = SafeModeController::new(config());
    let m = c.record_failure();
    assert_eq!(m, RuntimeMode::Normal);
    assert_eq!(c.consecutive_failures(), 1);
}

#[test]
fn second_failure_enters_degraded() {
    let c = SafeModeController::new(config());
    c.record_failure();
    let m = c.record_failure();
    assert_eq!(m, RuntimeMode::Degraded);
    assert_eq!(c.current_mode(), RuntimeMode::Degraded);
}

#[test]
fn fourth_failure_enters_safe() {
    let c = SafeModeController::new(config());
    for _ in 0..4 {
        c.record_failure();
    }
    assert_eq!(c.current_mode(), RuntimeMode::Safe);
}

#[test]
fn success_resets_failure_count() {
    let c = SafeModeController::new(config());
    c.record_failure();
    c.record_failure();
    assert_eq!(c.current_mode(), RuntimeMode::Degraded);
    c.record_success();
    assert_eq!(c.consecutive_failures(), 0);
    // 单次成功不足 recovery_threshold=2，仍为 Degraded
    assert_eq!(c.current_mode(), RuntimeMode::Degraded);
}

#[test]
fn two_consecutive_successes_recover_from_degraded() {
    let c = SafeModeController::new(config());
    c.record_failure();
    c.record_failure();
    assert_eq!(c.current_mode(), RuntimeMode::Degraded);

    c.record_success();
    let m = c.record_success();
    assert_eq!(m, RuntimeMode::Normal);
}

#[test]
fn recovers_from_safe_via_degraded() {
    let c = SafeModeController::new(config());
    for _ in 0..4 {
        c.record_failure();
    }
    assert_eq!(c.current_mode(), RuntimeMode::Safe);

    // Safe → Degraded
    c.record_success();
    let m = c.record_success();
    assert_eq!(m, RuntimeMode::Recovery);

    // Degraded → Normal
    c.record_success();
    let m = c.record_success();
    assert_eq!(m, RuntimeMode::Normal);
}

#[test]
fn reset_returns_to_normal() {
    let c = SafeModeController::new(config());
    for _ in 0..4 {
        c.record_failure();
    }
    assert_eq!(c.current_mode(), RuntimeMode::Safe);
    c.reset();
    assert_eq!(c.current_mode(), RuntimeMode::Normal);
    assert_eq!(c.consecutive_failures(), 0);
    assert_eq!(c.consecutive_successes(), 0);
}

#[test]
fn config_validation() {
    let mut cfg = AdaptiveLoopConfig::default();
    assert!(cfg.validate().is_ok());

    cfg.degraded_threshold = 0;
    assert!(cfg.validate().is_err());

    cfg = AdaptiveLoopConfig::default();
    cfg.safe_threshold = 0; // < degraded_threshold
    assert!(cfg.validate().is_err());

    cfg = AdaptiveLoopConfig::default();
    cfg.recovery_threshold = 0;
    assert!(cfg.validate().is_err());
}
