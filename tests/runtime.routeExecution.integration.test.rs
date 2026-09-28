//! Route Binder TickOutcome integration test.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    runtime::route::{RuntimeRoute, RuntimeRouteSource},
};
use aicore_contracts::{
    errors::ContractResult,
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    runtime::config::AdaptiveLoopConfig,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine,
        policy_executor::IPolicyExecutor,
        runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
        runtime_orchestrator::IRuntimeOrchestrator,
        runtime_router::IRuntimeRouter,
        telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::RuleBasedPolicyEngine,
};
use aicore_runtime::AdaptiveLoop;

struct MockTelemetry {
    calls: AtomicU64,
}

impl MockTelemetry {
    fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
        }
    }
}

impl ITelemetryProvider for MockTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

/// Fixed output of Cup Router.
struct FixedCpuRouter;
impl IRuntimeRouter for FixedCpuRouter {
    fn decide(&self, _s: &ComputerSnapshot, _c: &ContextSnapshot) -> ContractResult<RuntimeRoute> {
        Ok(RuntimeRoute {
            route_id: "route-fixed-cpu".to_string(),
            workload_type: WorkloadType::Development,
            source: RuntimeRouteSource::Cpu,
            applied: true,
            reason: "fixed: cpu".to_string(),
            confidence: 0.9,
            decided_at_unix_secs: 1_700_000_000,
        })
    }
    fn name(&self) -> &'static str {
        "fixed_cpu_router"
    }
}

/// Fixed returns the exported = true of Binder.
struct OkBinder;
impl IRuntimeBinder for OkBinder {
    fn bind_and_execute(&self, req: &BindingRequest) -> ContractResult<BindingResult> {
        Ok(BindingResult {
            route_id: req.route.route_id.clone(),
            source: req.route.source,
            executed: true,
            inference: None,
            reason: "test binder accepted".to_string(),
            latency_ms: 1,
        })
    }
    fn name(&self) -> &'static str {
        "ok_binder"
    }
    fn loaded_models(&self) -> Vec<aicore_contracts::ai::runtime::ModelHandle> {
        vec![]
    }
}

fn build_loop() -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(aicore_core::policy::DefaultPolicyValidator::new());
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
fn route_and_binding_recorded_in_outcome() {
    let loop_ = build_loop()
        .with_runtime_router(Arc::new(FixedCpuRouter))
        .with_runtime_binder(Arc::new(OkBinder));

    let outcome = loop_.tick(&ExecutionTargets::default());

    assert_eq!(outcome.runtime_route_source.as_deref(), Some("cpu"));
    assert_eq!(outcome.runtime_binding_executed, Some(true));
    assert!(outcome
        .runtime_binding_reason
        .as_deref()
        .unwrap()
        .contains("accepted"));
}

#[test]
fn no_router_means_no_route_fields() {
    let loop_ = build_loop();
    let outcome = loop_.tick(&ExecutionTargets::default());

    assert!(outcome.runtime_route_source.is_none());
    assert!(outcome.runtime_binding_executed.is_none());
    assert!(outcome.runtime_binding_reason.is_none());
}

#[test]
fn router_without_binder_records_route_only() {
    let loop_ = build_loop().with_runtime_router(Arc::new(FixedCpuRouter));
    let outcome = loop_.tick(&ExecutionTargets::default());

    assert_eq!(outcome.runtime_route_source.as_deref(), Some("cpu"));
    assert!(outcome.runtime_binding_executed.is_none());
}
