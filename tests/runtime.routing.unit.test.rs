//! SafeRuntimeRouter 单元测试。

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

fn ctx(w: WorkloadType, confidence: f32) -> ContextSnapshot {
    ContextSnapshot {
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        confidence,
        ..Default::default()
    }
}

fn snapshot_with(npu_available: bool, gpu_available: bool, cpu_usage: f32) -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.logical_cores = 8;
    s.cpu.usage_percent = cpu_usage;
    s.npu = NpuSnapshot {
        available: npu_available,
        usage_percent: Some(0.0),
        ..Default::default()
    };
    if gpu_available {
        s.gpu = GpuSnapshot {
            name: Some("Test GPU".to_string()),
            usage_percent: Some(0.0),
            ..Default::default()
        };
    }
    s
}

#[test]
fn router_name() {
    let r = SafeRuntimeRouter::new();
    assert_eq!(r.name(), "safe_runtime_router");
    assert!((r.min_confidence() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn unknown_workload_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(true, true, 20.0);
    let route = r.decide(&snap, &ctx(WorkloadType::Unknown, 0.9)).unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Native);
    assert!(!route.applied);
    assert!(route.reason.contains("unknown workload"));
}

#[test]
fn low_confidence_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(true, true, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.3))
        .unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Native);
    assert!(route.reason.contains("low confidence"));
}

#[test]
fn ai_inference_with_npu_routes_to_npu() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(true, false, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Npu);
    assert!(route.applied);
    assert!(route.reason.contains("NPU available"));
}

#[test]
fn ai_inference_without_npu_but_gpu_routes_to_gpu() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, true, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Gpu);
    assert!(route.reason.contains("NPU unavailable"));
    // GPU fallback 置信度 *0.8
    assert!(route.confidence < 0.9);
}

#[test]
fn ai_inference_without_accelerator_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, false, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();

    assert_eq!(route.source, RuntimeRouteSource::Native);
    assert!(route.reason.contains("no NPU/GPU"));
}

#[test]
fn rendering_with_gpu_routes_to_gpu() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, true, 20.0);
    let route = r.decide(&snap, &ctx(WorkloadType::Rendering, 0.9)).unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Gpu);
}

#[test]
fn video_editing_without_gpu_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, false, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::VideoEditing, 0.9))
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Native);
}

#[test]
fn compilation_with_idle_cpu_routes_to_cpu() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, false, 30.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::Compilation, 0.9))
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Cpu);
    assert!(route.reason.contains("CPU headroom"));
}

#[test]
fn compilation_with_busy_cpu_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, false, 90.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::Compilation, 0.9))
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Native);
    assert!(route.reason.contains("CPU busy"));
}

#[test]
fn development_keeps_native() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(true, true, 30.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::Development, 0.9))
        .unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Native);
}

#[test]
fn route_metadata_populated() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(true, false, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();

    assert!(route.route_id.starts_with("route-"));
    assert!(route.decided_at_unix_secs > 0);
}

#[test]
fn native_route_has_high_confidence() {
    let r = SafeRuntimeRouter::new();
    let snap = snapshot_with(false, false, 20.0);
    let route = r.decide(&snap, &ctx(WorkloadType::Unknown, 0.5)).unwrap();
    // Native 是"确定保持路径"，置信度应为 1.0
    assert_eq!(route.confidence, 1.0);
}

#[test]
fn router_with_custom_min_confidence() {
    let r = SafeRuntimeRouter::new().with_min_confidence(0.95);
    let snap = snapshot_with(true, false, 20.0);
    let route = r
        .decide(&snap, &ctx(WorkloadType::AiInference, 0.9))
        .unwrap();
    // 0.9 < 0.95 → Native
    assert_eq!(route.source, RuntimeRouteSource::Native);
}
