//! 故障恢复测试：AI/Adapter/IPC 失败时系统继续运行。

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

/// 前 N 次 collect 失败，之后成功。
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

    // 前 3 次失败
    for _ in 0..3 {
        let o = loop_.tick(&targets);
        assert!(!o.success);
    }

    // 第 4 次恢复
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

    // 5 次失败后应进入非 Normal 模式
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
    // metrics 一致
    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
}
