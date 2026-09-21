//! Rollback 集成测试：通过 AdaptiveLoop 走完六段式。

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use aicore_contracts::{
    errors::ContractResult,
    execution::{
        result::ExecutionResult, status::ExecutionOverallStatus, targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    runtime::config::AdaptiveLoopConfig,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
        transparent_executor::ITransparentExecutor,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    executor::TransparentExecutor,
    policy::RuleBasedPolicyEngine,
};
use aicore_runtime::AdaptiveLoop;

struct MockTelemetry;

impl ITelemetryProvider for MockTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        let mut s = ComputerSnapshot::default();
        s.cpu.logical_cores = 8;
        Ok(s)
    }
    fn platform_name(&self) -> &'static str {
        "mock"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// 第 1 次执行返回 AllFailed，后续返回 AllSucceeded。
struct FailThenSucceedExecutor {
    calls: AtomicU32,
}

impl IPolicyExecutor for FailThenSucceedExecutor {
    fn execute(&self, p: &ResourcePolicy, _t: &ExecutionTargets) -> ExecutionResult {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let status = if n == 0 {
            ExecutionOverallStatus::AllFailed
        } else {
            ExecutionOverallStatus::AllSucceeded
        };
        ExecutionResult {
            policy_id: p.policy_id.clone(),
            overall_status: status,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: Vec::new(),
        }
    }
    fn name(&self) -> &'static str {
        "fail_then_succeed"
    }
}

/// 构造 AdaptiveLoop，同时把 inner executor 给 loop 的 executor 槽，
/// 把 TransparentExecutor 给 transparent 槽。
///
/// 注意：两者共享同一个底层实现（通过 Arc clone），这样 tick 时
/// 若 transparent 路径触发回退，回退会复用同一个底层。
fn build_loop_with_transparent(
    inner: Arc<dyn IPolicyExecutor>,
    transparent: Arc<dyn ITransparentExecutor>,
) -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry);
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());

    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        inner,
        AdaptiveLoopConfig::default(),
    )
    .with_transparent_executor(transparent)
}

#[test]
fn first_tick_without_prior_rolls_back_failed() {
    let inner: Arc<dyn IPolicyExecutor> = Arc::new(FailThenSucceedExecutor {
        calls: AtomicU32::new(0),
    });

    // 用一个独立的 TransparentExecutor 包装同类型的 inner
    let inner_for_transparent: Arc<dyn IPolicyExecutor> = Arc::new(FailThenSucceedExecutor {
        calls: AtomicU32::new(0),
    });
    let tx: Arc<dyn ITransparentExecutor> =
        Arc::new(TransparentExecutor::new(inner_for_transparent));

    let loop_ = build_loop_with_transparent(inner, tx);
    let outcome = loop_.tick(&ExecutionTargets::default());

    // 首次 tick：无 prior → rollback 失败；status = AllFailed
    assert!(!outcome.success);
}

#[test]
fn second_tick_runs_without_panic() {
    let inner: Arc<dyn IPolicyExecutor> = Arc::new(FailThenSucceedExecutor {
        calls: AtomicU32::new(0),
    });
    let inner_for_transparent: Arc<dyn IPolicyExecutor> = Arc::new(FailThenSucceedExecutor {
        calls: AtomicU32::new(0),
    });
    let tx: Arc<dyn ITransparentExecutor> =
        Arc::new(TransparentExecutor::new(inner_for_transparent));

    let loop_ = build_loop_with_transparent(inner, tx);

    let o1 = loop_.tick(&ExecutionTargets::default());
    let o2 = loop_.tick(&ExecutionTargets::default());

    assert_eq!(o1.tick_id, 1);
    assert_eq!(o2.tick_id, 2);
}

#[test]
fn adaptive_loop_without_transparent_uses_v2_path() {
    let inner: Arc<dyn IPolicyExecutor> = Arc::new(FailThenSucceedExecutor {
        calls: AtomicU32::new(0),
    });

    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry);
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());

    // 未启用 transparent
    let loop_ = AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        inner,
        AdaptiveLoopConfig::default(),
    );

    let outcome = loop_.tick(&ExecutionTargets::default());
    // V2 路径：第 1 次失败
    assert!(!outcome.success);
    assert_eq!(outcome.tick_id, 1);
}
