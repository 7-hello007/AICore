//! L0 独立测试：注册、观察、建议。

use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    telemetry::{
        application::ApplicationSnapshot, gpu::GpuSnapshot, npu::NpuSnapshot,
        process::ProcessSnapshot, snapshot::ComputerSnapshot,
    },
    traits::{adaptation_provider::IAdaptationProvider, workload_source::IWorkloadSource},
};
use aicore_tier::TierHub;
use aicore_tier_l0::{register_l0, L0AdaptationProvider, L0WorkloadSource, L0_TIER_ID};

fn snapshot_with_app(name: &str) -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.logical_cores = 8;
    s.applications = vec![ApplicationSnapshot {
        name: name.to_string(),
        pid: 1000,
        is_foreground: true,
        window_title: Some(format!("{} window", name)),
    }];
    s.top_processes = vec![ProcessSnapshot {
        pid: 1000,
        name: name.to_string(),
        cpu_usage_percent: 10.0,
        memory_mb: 100,
        is_foreground: true,
    }];
    s
}

#[test]
fn workload_source_detects_development() {
    let source = L0WorkloadSource::new();
    let snap = snapshot_with_app("Code");
    let obs = source.observe(&snap).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);
    assert!(obs.confidence > 0.5);
    assert_eq!(source.source_name(), "l0.workload_source");
    assert_eq!(source.priority(), 100);
}

#[test]
fn workload_source_detects_ai_inference() {
    let source = L0WorkloadSource::new();
    let snap = snapshot_with_app("ollama");
    let obs = source.observe(&snap).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::AiInference);
}

#[test]
fn workload_source_returns_unknown_for_empty() {
    let source = L0WorkloadSource::new();
    let obs = source.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Unknown);
    assert_eq!(obs.confidence, 0.0);
}

#[test]
fn adaptation_provider_returns_none_for_unknown() {
    let provider = L0AdaptationProvider::new();
    let snap = ComputerSnapshot::default();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::Unknown,
        interaction_level: InteractionLevel::Idle,
        ..Default::default()
    };
    let hint = provider.suggest(&snap, &ctx).unwrap();
    assert!(hint.is_none());
}

#[test]
fn adaptation_provider_returns_hint_for_known() {
    let provider = L0AdaptationProvider::new();
    let snap = snapshot_with_app("Code");
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Interactive,
        confidence: 0.9,
        ..Default::default()
    };
    let hint = provider.suggest(&snap, &ctx).unwrap().unwrap();
    assert!(hint.policy_override.is_some());
    assert!(hint.confidence >= 0.5);
    assert!(hint.reason.contains("l0"));
    assert_eq!(provider.source_name(), "l0.adaptation_provider");
}

#[test]
fn adaptation_provider_downgrades_gpu_when_unavailable() {
    let provider = L0AdaptationProvider::new();
    let mut snap = snapshot_with_app("Code");
    snap.gpu = GpuSnapshot::default(); // usage_percent = None
    snap.npu = NpuSnapshot::default(); // available = false

    let ctx = ContextSnapshot {
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Interactive,
        confidence: 0.9,
        ..Default::default()
    };

    let hint = provider.suggest(&snap, &ctx).unwrap().unwrap();
    let policy = hint.policy_override.unwrap();
    // GPU / NPU 不可用 → 优先级降级
    assert_eq!(
        policy.gpu.priority,
        aicore_contracts::policy::priority::PolicyPriority::Normal
    );
    assert_eq!(
        policy.npu.priority,
        aicore_contracts::policy::priority::PolicyPriority::Normal
    );
    assert!(!policy.npu.preferred);
    // reason 里应有降级说明
    assert!(hint.reason.contains("gpu_unavailable"));
    assert!(hint.reason.contains("npu_unavailable"));
}

#[test]
fn register_l0_succeeds() {
    let hub = TierHub::new();
    let id = register_l0(&hub).unwrap();
    assert_eq!(id.as_str(), L0_TIER_ID);

    assert!(hub.best_workload_source().unwrap().is_some());
    assert!(hub.best_adaptation_provider().unwrap().is_some());
    assert_eq!(hub.registry().count().unwrap(), 1);
}

#[test]
fn register_l0_twice_fails() {
    let hub = TierHub::new();
    register_l0(&hub).unwrap();
    assert!(register_l0(&hub).is_err());
}

#[test]
fn empty_hub_still_valid_without_l0() {
    let hub = TierHub::new();
    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.best_adaptation_provider().unwrap().is_none());
}
