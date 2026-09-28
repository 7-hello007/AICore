//! MlWorkloadSource unit test.

use aicore_ai::workload::ml_source::{MlWeightTable, FEATURE_DIM};
use aicore_ai::MlWorkloadSource;
use aicore_contracts::{
    context::workload::WorkloadType,
    telemetry::snapshot::ComputerSnapshot,
    traits::workload_source::{IWorkloadSource, SourceKind},
};

fn snapshot(cpu: f32, gpu: f32, npu: f32, mem: f32) -> ComputerSnapshot {
    let mut s = ComputerSnapshot::default();
    s.cpu.usage_percent = cpu;
    s.gpu.usage_percent = Some(gpu);
    s.npu.usage_percent = Some(npu);
    s.memory.usage_percent = mem;
    s
}

#[test]
fn default_weights_valid() {
    let w = MlWeightTable::default_weights();
    assert!(w.validate().is_ok());
    assert_eq!(FEATURE_DIM, 4);
}

#[test]
fn provider_reports_kind_ml() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    assert_eq!(p.kind(), SourceKind::Ml);
    assert_eq!(p.priority(), 50);
    assert_eq!(p.source_name(), "ml.workload_source");
}

#[test]
fn high_npu_infers_ai_inference() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let obs = p.observe(&snapshot(20.0, 20.0, 90.0, 40.0)).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::AiInference);
}

#[test]
fn high_gpu_cpu_infers_gaming() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let obs = p.observe(&snapshot(80.0, 90.0, 0.0, 40.0)).unwrap();
    // High CPU + High GPU
    assert!(matches!(
        obs.workload_type,
        WorkloadType::Gaming | WorkloadType::Rendering
    ));
}

#[test]
fn low_everything_infers_background_or_office() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let obs = p.observe(&snapshot(5.0, 5.0, 0.0, 20.0)).unwrap();
    assert!(matches!(
        obs.workload_type,
        WorkloadType::GeneralOffice | WorkloadType::BackgroundTask
    ));
}

#[test]
fn empty_snapshot_does_not_panic() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let obs = p.observe(&ComputerSnapshot::default()).unwrap();
    // Not Panic; results depend on weight
    assert!(obs.confidence >= 0.0 && obs.confidence <= 1.0);
}

#[test]
fn output_is_deterministic() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let snap = snapshot(60.0, 60.0, 20.0, 50.0);
    let o1 = p.observe(&snap).unwrap();
    let o2 = p.observe(&snap).unwrap();
    assert_eq!(o1.workload_type, o2.workload_type);
    assert_eq!(o1.confidence, o2.confidence);
}

#[test]
fn evidence_includes_features() {
    let p = MlWorkloadSource::with_default_weights().unwrap();
    let obs = p.observe(&snapshot(80.0, 50.0, 10.0, 40.0)).unwrap();
    assert!(obs.evidence.iter().any(|e| e.contains("ml_features")));
    assert!(obs.evidence.iter().any(|e| e.contains("ml_prediction")));
}

#[test]
fn invalid_weights_rejected() {
    let mut w = MlWeightTable::default_weights();
    w.entries.clear();
    assert!(MlWorkloadSource::new(w).is_err());
}
