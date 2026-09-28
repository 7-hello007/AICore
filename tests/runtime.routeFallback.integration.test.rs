//! Runtime route fallback test:
//! - No available resources to keep native.
//! - Unknown workload hold native.

use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    runtime::route::RuntimeRouteSource,
    telemetry::{gpu::GpuSnapshot, npu::NpuSnapshot, snapshot::ComputerSnapshot},
    traits::runtime_router::IRuntimeRouter,
};
use aicore_runtime_router::SafeRuntimeRouter;

fn snapshot_bare() -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.logical_cores = 8;
    s
}

fn ctx(w: WorkloadType, confidence: f32) -> ContextSnapshot {
    ContextSnapshot {
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        confidence,
        ..Default::default()
    }
}

#[test]
fn all_accelerators_missing_still_safe() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_bare();

    for workload in [
        WorkloadType::AiInference,
        WorkloadType::Rendering,
        WorkloadType::VideoEditing,
        WorkloadType::Gaming,
    ] {
        let route = r.decide(&snap, &ctx(workload, 0.95)).unwrap();
        assert_eq!(
            route.source,
            RuntimeRouteSource::Native,
            "workload {:?} should keep native when no accelerators",
            workload
        );
    }
}

#[test]
fn gpu_present_but_name_only_still_counts() {
    let r = SafeRuntimeRouter::new();
    let mut snap = snapshot_bare();
    snap.gpu = GpuSnapshot {
        name: Some("gpu".to_string()),
        ..Default::default()
    };

    let route = r.decide(&snap, &ctx(WorkloadType::Rendering, 0.9)).unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Gpu);
}

#[test]
fn npu_available_but_usage_none_still_routes_to_npu() {
    let r = SafeRuntimeRouter::new();
    let mut snap = snapshot_bare();
    snap.npu = NpuSnapshot {
        available: true,
        usage_percent: None,
        ..Default::default()
    };

    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Npu);
}

#[test]
fn native_route_reports_not_applied() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_bare();
    let route = r.decide(&snap, &ctx(WorkloadType::Gaming, 0.9)).unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Native);
    assert!(!route.applied, "native route must not be marked as applied");
    assert!(route.is_native());
}

#[test]
fn routed_decision_has_non_native_source() {
    let r = SafeRuntimeRouter::new();
    let mut snap = snapshot_bare();
    snap.npu = NpuSnapshot {
        available: true,
        usage_percent: Some(0.0),
        ..Default::default()
    };

    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();
    assert!(route.applied);
    assert!(!route.is_native());
    assert_eq!(route.source, RuntimeRouteSource::Npu);
}

#[test]
fn route_is_deterministic() {
    let r = SafeRuntimeRouter::new();
    let mut snap = snapshot_bare();
    snap.npu = NpuSnapshot {
        available: true,
        usage_percent: Some(0.0),
        ..Default::default()
    };

    let a = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();
    let b = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();
    assert_eq!(a.source, b.source);
    assert_eq!(a.applied, b.applied);
    assert_eq!(a.confidence, b.confidence);
}
