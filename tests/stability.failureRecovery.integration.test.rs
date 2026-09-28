//! Failure recovery test: AI/Adapter/IPC continues to run when the system fails.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    runtime::config::AdaptiveLoopConfig,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
};
use aicore_runtime::AdaptiveLoop;

/// The first N-collect failed, then succeeded.
struct FlakyTelemetry {
    fail_count: AtomicU64,
}

impl FlakyTelemetry {
    fn new(fail_first: u64) -> Self {
        Self {
            fail_count: AtomicU64::new(fail_first),
        }
    }
}

impl ITelemetryProvider for FlakyTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        if self.fail_count.load(Ordering::SeqCst) > 0 {
            self.fail_count.fetch_sub(1, Ordering::SeqCst);
            return Err(ContractError::ResourceUnavailable(
                "simulated telemetry failure".into(),
            ));
        }
        let mut s = ComputerSnapshot::default();
        s.cpu.logical_cores = 8;
        Ok(s)
    }
    fn platform_name(&self) -> &'static str {
        "flaky"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
}

struct OkExecutor;
impl IPolicyExecutor for OkExecutor {
    fn execute(&self, p: &ResourcePolicy, _t: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: p.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "test".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::Success,
                message: "ok".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "ok"
    }
}

fn build_loop(telemetry: Arc<dyn ITelemetryProvider>) -> AdaptiveLoop {
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(DefaultPolicyValidator::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(OkExecutor);

    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
}

#[test]
fn telemetry_failure_does_not_panic() {
    let telemetry = Arc::new(FlakyTelemetry::new(3));
    let loop_ = build_loop(telemetry);
    let targets = ExecutionTargets::default();

    // Previous 3 Failures
    for _ in 0..3 {
        let o = loop_.tick(&targets);
        assert!(!o.success);
    }

    // Fourth resumption
    let o = loop_.tick(&targets);
    assert!(o.success, "expected recovery: {}", o.message);
}

#[test]
fn safe_mode_transitions_after_repeated_failures() {
    let telemetry = Arc::new(FlakyTelemetry::new(100));
    let loop_ = build_loop(telemetry);
    let targets = ExecutionTargets::default();

    for _ in 0..5 {
        let _ = loop_.tick(&targets);
    }

    // 5 failed should enter non-Normal mode
    let mode = loop_.current_mode();
    assert_ne!(
        mode,
        aicore_contracts::runtime::mode::RuntimeMode::Normal,
        "expected non-Normal after 5 failures"
    );
}

#[test]
fn recovery_after_failure_does_not_crash() {
    let telemetry = Arc::new(FlakyTelemetry::new(2));
    let loop_ = build_loop(telemetry);
    let targets = ExecutionTargets::default();

    let _ = loop_.tick(&targets);
    let _ = loop_.tick(&targets);
    let o = loop_.tick(&targets);

    assert!(o.success);
    // Metrics
    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
}
