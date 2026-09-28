//! Layered workload inference integration: rules + ML + fusion.

use std::sync::Arc;

use aicore_ai::MlWorkloadSource;
use aicore_contracts::{
    context::workload::WorkloadType,
    telemetry::{
        application::ApplicationSnapshot, process::ProcessSnapshot, snapshot::ComputerSnapshot,
    },
    traits::workload_source::IWorkloadSource,
};
use aicore_core::context::WorkloadFusion;
use aicore_tier::TierHub;
use aicore_tier_l0::{register_l0_with_fusion, L0_TIER_ID};

fn dev_snapshot() -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.logical_cores = 8;
    s.applications = vec![ApplicationSnapshot {
        name: "Code".to_string(),
        pid: 1000,
        is_foreground: true,
        window_title: Some("main.rs".to_string()),
    }];
    s.top_processes = vec![ProcessSnapshot {
        pid: 1000,
        name: "code".to_string(),
        cpu_usage_percent: 10.0,
        memory_mb: 500,
        is_foreground: true,
    }];
    s
}

fn ai_snapshot() -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.logical_cores = 8;
    s.cpu.usage_percent = 20.0;
    s.gpu.usage_percent = Some(20.0);
    s.npu.usage_percent = Some(90.0);
    s.memory.usage_percent = 40.0;
    s
}

#[test]
fn fusion_combines_rule_and_ml() {
    let mut fusion = WorkloadFusion::new();
    // Rule source (L0)
    fusion.add_source(Arc::new(aicore_tier_l0::L0WorkloadSource::new()));
    // ML Source
    fusion.add_source(Arc::new(MlWorkloadSource::with_default_weights().unwrap()));

    // A foreground Code application is classified as Development by the rule source.
    let obs = fusion.observe(&dev_snapshot()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);
}

#[test]
fn fusion_uses_ml_when_no_rule_signal() {
    let mut fusion = WorkloadFusion::new();
    fusion.add_source(Arc::new(aicore_tier_l0::L0WorkloadSource::new()));
    fusion.add_source(Arc::new(MlWorkloadSource::with_default_weights().unwrap()));

    // No foreground application, but high NPU usage suggests AI inference to ML.
    let obs = fusion.observe(&ai_snapshot()).unwrap();
    // A single ML source may have confidence below 0.9, yielding Unknown or AI Inference.
    // Check that inference does not panic and returns a valid result.
    assert!(matches!(
        obs.workload_type,
        WorkloadType::AiInference | WorkloadType::Unknown
    ));
}

#[test]
fn register_l0_with_fusion_and_query() {
    let hub = TierHub::new();
    let ml: Arc<dyn IWorkloadSource> = Arc::new(MlWorkloadSource::with_default_weights().unwrap());

    let id = register_l0_with_fusion(&hub, vec![ml]).unwrap();
    assert_eq!(id.as_str(), L0_TIER_ID);

    let source = hub.best_workload_source().unwrap().unwrap();
    assert_eq!(source.source_name(), "workload_fusion");

    // The source registered through the hub works as well.
    let obs = source.observe(&dev_snapshot()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);
}

#[test]
fn remove_l0_with_fusion_keeps_empty_hub_valid() {
    let hub = TierHub::new();
    let ml: Arc<dyn IWorkloadSource> = Arc::new(MlWorkloadSource::with_default_weights().unwrap());

    let id = register_l0_with_fusion(&hub, vec![ml]).unwrap();
    hub.unregister_tier(&id).unwrap();

    assert!(hub.best_workload_source().unwrap().is_none());
    assert!(hub.registry().is_empty().unwrap());
}
