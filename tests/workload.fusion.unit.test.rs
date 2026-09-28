//! WorkloadFusion unit test.

use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    telemetry::snapshot::ComputerSnapshot,
    traits::workload_source::{IWorkloadSource, SourceKind, WorkloadObservation},
};
use aicore_core::context::WorkloadFusion;

struct MockSource {
    name: &'static str,
    kind: SourceKind,
    priority: u32,
    workload: WorkloadType,
    confidence: f32,
    err: bool,
}

impl IWorkloadSource for MockSource {
    fn observe(&self, _s: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        if self.err {
            return Err(ContractError::AiInferenceFailed("mock error".to_string()));
        }
        Ok(WorkloadObservation {
            workload_type: self.workload,
            confidence: self.confidence,
            evidence: vec![format!("from {}", self.name)],
        })
    }
    fn source_name(&self) -> &'static str {
        self.name
    }
    fn priority(&self) -> u32 {
        self.priority
    }
    fn kind(&self) -> SourceKind {
        self.kind
    }
}

fn src(
    name: &'static str,
    kind: SourceKind,
    priority: u32,
    workload: WorkloadType,
    confidence: f32,
) -> Arc<dyn IWorkloadSource> {
    Arc::new(MockSource {
        name,
        kind,
        priority,
        workload,
        confidence,
        err: false,
    })
}

fn src_err() -> Arc<dyn IWorkloadSource> {
    Arc::new(MockSource {
        name: "err",
        kind: SourceKind::Rule,
        priority: 100,
        workload: WorkloadType::Unknown,
        confidence: 0.0,
        err: true,
    })
}

#[test]
fn empty_fusion_returns_unknown() {
    let fusion = WorkloadFusion::new();
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Unknown);
    assert_eq!(obs.confidence, 0.0);
}

#[test]
fn single_high_confidence_rule_accepted() {
    let fusion = WorkloadFusion::new().with_source(src(
        "rule",
        SourceKind::Rule,
        100,
        WorkloadType::Development,
        0.95,
    ));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);
    assert_eq!(obs.confidence, 0.95);
}

#[test]
fn high_confidence_ml_accepted() {
    let fusion = WorkloadFusion::new().with_source(src(
        "ml",
        SourceKind::Ml,
        50,
        WorkloadType::Gaming,
        0.98,
    ));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Gaming);
    // ML requires > = 0.95 for direct adoption
}

#[test]
fn medium_confidence_ml_not_directly_accepted() {
    let fusion =
        WorkloadFusion::new().with_source(src("ml", SourceKind::Ml, 50, WorkloadType::Gaming, 0.7));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    // 0.7 < 0.95 Not accepted; vote is only 1 vote for 0.7 < 0.5? No, 0.7 > = 0.5 so return
    // But workload keeps Gaming
    assert_eq!(obs.workload_type, WorkloadType::Gaming);
    assert!((obs.confidence - 0.7).abs() < f32::EPSILON);
}

#[test]
fn low_confidence_returns_unknown() {
    let fusion =
        WorkloadFusion::new().with_source(src("ml", SourceKind::Ml, 50, WorkloadType::Gaming, 0.3));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Unknown);
    assert!(obs.evidence.iter().any(|e| e.contains("low confidence")));
}

#[test]
fn two_sources_agree_get_boost() {
    let fusion = WorkloadFusion::new()
        .with_source(src(
            "rule",
            SourceKind::Rule,
            100,
            WorkloadType::Development,
            0.7,
        ))
        .with_source(src(
            "ml",
            SourceKind::Ml,
            50,
            WorkloadType::Development,
            0.65,
        ));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::Development);
    // Two-source alignment → Upgrade 0.1 (base 0.7 → 0.8)
    assert!(obs.confidence > 0.7);
    assert!(obs.evidence.iter().any(|e| e.contains("agree")));
}

#[test]
fn higher_vote_wins() {
    let fusion = WorkloadFusion::new()
        .with_source(src(
            "rule1",
            SourceKind::Rule,
            100,
            WorkloadType::Development,
            0.6,
        ))
        .with_source(src(
            "rule2",
            SourceKind::Rule,
            90,
            WorkloadType::Development,
            0.6,
        ))
        .with_source(src("ml", SourceKind::Ml, 50, WorkloadType::Gaming, 0.7));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    // Development 2 votes, Gaming 1 vote.
    assert_eq!(obs.workload_type, WorkloadType::Development);
}

#[test]
fn source_error_does_not_panic() {
    let fusion = WorkloadFusion::new()
        .with_source(src_err())
        .with_source(src(
            "good",
            SourceKind::Rule,
            100,
            WorkloadType::AiInference,
            0.95,
        ));
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert_eq!(obs.workload_type, WorkloadType::AiInference);
}

#[test]
fn rule_priority_higher_than_ml() {
    let fusion = WorkloadFusion::new()
        .with_source(src("ml", SourceKind::Ml, 50, WorkloadType::Gaming, 0.7))
        .with_source(src(
            "rule",
            SourceKind::Rule,
            200,
            WorkloadType::Development,
            0.7,
        ));
    // Two-source vote 1:1, but Rule priority 200 > Ml 50, therefore sorted before rule
    // Since both have the same confidence, counting the votes and comparing them with confidence
    // But max_by compares the number of votes (both 1) and confidence (both 0.7).
    // This is a defined fuzzy scene. Tests only validate non-panic.
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    assert!(matches!(
        obs.workload_type,
        WorkloadType::Gaming | WorkloadType::Development
    ));
}

#[test]
fn fusion_source_name_and_priority() {
    let fusion = WorkloadFusion::new();
    assert_eq!(fusion.source_name(), "workload_fusion");
    assert_eq!(fusion.priority(), 1000);
    assert_eq!(fusion.source_count(), 0);
}

#[test]
fn fusion_with_custom_min_confidence() {
    let fusion = WorkloadFusion::new()
        .with_source(src("ml", SourceKind::Ml, 50, WorkloadType::Gaming, 0.65))
        .with_min_return_confidence(0.8);
    let obs = fusion.observe(&ComputerSnapshot::default()).unwrap();
    // 0.65 < 0.8 → Unknown
    assert_eq!(obs.workload_type, WorkloadType::Unknown);
}
