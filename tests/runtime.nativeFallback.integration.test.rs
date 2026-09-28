//! Runtime route fallback test: Backend is not available.

use aicore_contracts::{
    ai::runtime::ModelHandle,
    errors::ContractResult,
    runtime::route::{RuntimeRoute, RuntimeRouteSource},
    traits::{
        runtime_binder::{BindingRequest, IRuntimeBinder},
        runtime_router::IRuntimeRouter,
    },
};
use aicore_contracts::{context::snapshot::ContextSnapshot, telemetry::snapshot::ComputerSnapshot};
use aicore_runtime_router::DefaultRuntimeBinder;

/// Fixed output of Gpu route Router.
struct FixedGpuRouter;
impl IRuntimeRouter for FixedGpuRouter {
    fn decide(&self, _s: &ComputerSnapshot, _c: &ContextSnapshot) -> ContractResult<RuntimeRoute> {
        Ok(RuntimeRoute {
            route_id: "route-gpu".to_string(),
            workload_type: aicore_contracts::context::workload::WorkloadType::Rendering,
            source: RuntimeRouteSource::Gpu,
            applied: true,
            reason: "test".to_string(),
            confidence: 0.9,
            decided_at_unix_secs: 1_700_000_000,
        })
    }
    fn name(&self) -> &'static str {
        "fixed_gpu_router"
    }
}

/// Checks whether DefaultRuntimeBinder against Gpu route returns.
#[test]
fn default_binder_falls_back_for_gpu() {
    let binder = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: RuntimeRoute {
            route_id: "r1".to_string(),
            workload_type: aicore_contracts::context::workload::WorkloadType::Rendering,
            source: RuntimeRouteSource::Gpu,
            applied: true,
            reason: "test".to_string(),
            confidence: 0.9,
            decided_at_unix_secs: 0,
        },
        context: ContextSnapshot::default(),
        inference: None,
    };
    let res = binder.bind_and_execute(&req).unwrap();
    assert!(
        !res.executed,
        "GPU route must fall back to Native in Phase 3"
    );
    assert!(res.reason.contains("no GPU capability"));
}

#[test]
fn default_binder_falls_back_for_npu() {
    let binder = DefaultRuntimeBinder::new();
    let req = BindingRequest {
        route: RuntimeRoute {
            route_id: "r2".to_string(),
            workload_type: aicore_contracts::context::workload::WorkloadType::AiInference,
            source: RuntimeRouteSource::Npu,
            applied: true,
            reason: "test".to_string(),
            confidence: 0.9,
            decided_at_unix_secs: 0,
        },
        context: ContextSnapshot::default(),
        inference: None,
    };
    let res = binder.bind_and_execute(&req).unwrap();
    assert!(!res.executed);
    assert!(res.reason.contains("no NPU capability"));
}

#[test]
fn fixed_gpu_router_recorded_but_falls_back() {
    // Directly use Router + Binder (not AdaptiveLoop)
    let router = FixedGpuRouter;
    let binder = DefaultRuntimeBinder::new();

    let route = router
        .decide(&ComputerSnapshot::default(), &ContextSnapshot::default())
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Gpu);

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: route.clone(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();
    assert!(!binding.executed, "GPU fallback");
}

#[test]
fn native_route_produces_no_binding_work() {
    let binder = DefaultRuntimeBinder::new();
    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: RuntimeRoute {
                route_id: "r3".to_string(),
                workload_type: aicore_contracts::context::workload::WorkloadType::GeneralOffice,
                source: RuntimeRouteSource::Native,
                applied: false,
                reason: "native".to_string(),
                confidence: 1.0,
                decided_at_unix_secs: 0,
            },
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();
    assert!(!binding.executed);
    assert!(binding.reason.contains("native"));
    assert!(binding.inference.is_none());
}

#[test]
fn binder_returns_empty_loaded_models_by_default() {
    let binder = DefaultRuntimeBinder::new();
    let models: Vec<ModelHandle> = binder.loaded_models();
    assert!(models.is_empty());
}
