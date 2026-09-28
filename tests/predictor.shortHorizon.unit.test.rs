//! ShortHorizonPredictor unit test.

use aicore_contracts::{
    application::id::ApplicationId,
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    errors::ContractError,
    memory::context::{ApplicationHourlyPattern, HourlyWorkloadPattern, MemoryContext},
    prediction::input::PredictionInput,
    telemetry::snapshot::ComputerSnapshot,
    traits::predictor::IPredictor,
};
use aicore_core::prediction::ShortHorizonPredictor;

fn input_with_memory(memory: Option<MemoryContext>) -> PredictionInput {
    PredictionInput {
        application_id: None,
        snapshot: ComputerSnapshot::default(),
        context: ContextSnapshot::default(),
        memory,
        horizon_ms: 5_000,
    }
}

#[test]
fn registered_application_pattern_can_drive_prediction_without_guessing_identity() {
    let mut input = input_with_memory(Some(MemoryContext {
        recent_workloads: vec![WorkloadType::Gaming; 3],
        dominant_workload: Some(WorkloadType::Gaming),
        preferred_power_mode: None,
        sample_count: 3,
        hourly_patterns: Vec::new(),
        application_patterns: vec![ApplicationHourlyPattern {
            application_id: ApplicationId::new("app.editor"),
            hour_utc: 14,
            dominant_workload: WorkloadType::Development,
            sample_count: 3,
            confidence: 1.0,
        }],
    }));
    input.snapshot.collected_at_unix_ms = Some(14 * 3_600_000);
    let predictor = ShortHorizonPredictor::with_defaults();
    assert_eq!(
        predictor.predict(&input).unwrap().predicted_workload,
        WorkloadType::Gaming
    );
    input.application_id = Some(ApplicationId::new("app.editor"));
    assert_eq!(
        predictor.predict(&input).unwrap().predicted_workload,
        WorkloadType::Development
    );
}

fn consistent_memory(workload: WorkloadType, samples: usize) -> MemoryContext {
    MemoryContext {
        recent_workloads: vec![workload; samples.min(8)],
        dominant_workload: Some(workload),
        preferred_power_mode: None,
        sample_count: samples,
        hourly_patterns: Vec::new(),
        application_patterns: Vec::new(),
    }
}

#[test]
fn predict_without_memory_fails() {
    let p = ShortHorizonPredictor::with_defaults();
    let err = p.predict(&input_with_memory(None)).unwrap_err();
    assert!(matches!(err, ContractError::AiInferenceFailed(_)));
}

#[test]
fn predict_with_insufficient_samples_fails() {
    let p = ShortHorizonPredictor::new(5, 5_000, 0.5);
    let mem = MemoryContext {
        recent_workloads: vec![WorkloadType::Development; 2],
        dominant_workload: Some(WorkloadType::Development),
        preferred_power_mode: None,
        sample_count: 2,
        hourly_patterns: Vec::new(),
        application_patterns: Vec::new(),
    };
    let err = p.predict(&input_with_memory(Some(mem))).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("insufficient"), "got: {}", msg);
}

#[test]
fn predict_with_consistent_samples_succeeds() {
    let p = ShortHorizonPredictor::new(3, 5_000, 0.5);
    let mem = consistent_memory(WorkloadType::Development, 5);
    let pred = p.predict(&input_with_memory(Some(mem))).unwrap();
    assert_eq!(pred.predicted_workload, WorkloadType::Development);
    assert!(pred.confidence >= 0.5);
    assert_eq!(pred.predictor_name, "short_horizon");
    assert_eq!(pred.horizon_ms, 5_000);
}

#[test]
fn confidence_grows_with_samples() {
    let p = ShortHorizonPredictor::new(3, 5_000, 0.5);

    let few = consistent_memory(WorkloadType::Development, 5);
    let many = consistent_memory(WorkloadType::Development, 20);

    let c_few = p.predict(&input_with_memory(Some(few))).unwrap().confidence;
    let c_many = p
        .predict(&input_with_memory(Some(many)))
        .unwrap()
        .confidence;

    assert!(c_many > c_few, "expected {} > {}", c_many, c_few);
}

#[test]
fn predict_with_mixed_recent_falls_back_to_dominant() {
    let p = ShortHorizonPredictor::new(3, 5_000, 0.5);
    let mem = MemoryContext {
        recent_workloads: vec![
            WorkloadType::Gaming,
            WorkloadType::Development,
            WorkloadType::VideoEditing,
        ],
        dominant_workload: Some(WorkloadType::Development),
        preferred_power_mode: None,
        sample_count: 10,
        hourly_patterns: Vec::new(),
        application_patterns: Vec::new(),
    };
    let pred = p.predict(&input_with_memory(Some(mem))).unwrap();
    assert_eq!(pred.predicted_workload, WorkloadType::Development);
    // Most scene confidence is 0.5.
    assert!((pred.confidence - 0.5).abs() < f32::EPSILON);
}

#[test]
fn predict_with_unknown_dominant_fails() {
    let p = ShortHorizonPredictor::new(3, 5_000, 0.5);
    let mem = MemoryContext {
        recent_workloads: vec![WorkloadType::Unknown; 5],
        dominant_workload: Some(WorkloadType::Unknown),
        preferred_power_mode: None,
        sample_count: 5,
        hourly_patterns: Vec::new(),
        application_patterns: Vec::new(),
    };
    // Unknown and a sample is enough.
    let err = p.predict(&input_with_memory(Some(mem))).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("no dominant or consistent"), "got: {}", msg);
}

#[test]
fn min_confidence_filters_low_confidence() {
    let p = ShortHorizonPredictor::new(3, 5_000, 0.99);
    let mem = consistent_memory(WorkloadType::Development, 5);
    // Consistency scene confidence ceiling 0.95, less than 0.99 will go to dominant branch
    // Dominant confidence 0.5 < 0.99 → Final Err
    let err = p.predict(&input_with_memory(Some(mem))).unwrap_err();
    assert!(matches!(err, ContractError::AiInferenceFailed(_)));
}

#[test]
fn name_and_horizon() {
    let p = ShortHorizonPredictor::new(3, 10_000, 0.5);
    assert_eq!(p.name(), "short_horizon");
    assert_eq!(p.horizon_ms(), 10_000);
    assert_eq!(p.min_samples(), 3);
    assert!((p.min_confidence() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn time_pattern_predicts_when_recent_sequence_is_mixed() {
    let predictor = ShortHorizonPredictor::with_defaults();
    let mut memory = consistent_memory(WorkloadType::Development, 8);
    memory.recent_workloads = vec![
        WorkloadType::Development,
        WorkloadType::Gaming,
        WorkloadType::Development,
    ];
    memory.hourly_patterns = vec![HourlyWorkloadPattern {
        hour_utc: 14,
        dominant_workload: WorkloadType::Gaming,
        sample_count: 8,
        confidence: 0.75,
    }];
    let mut input = input_with_memory(Some(memory));
    input.snapshot.collected_at_unix_ms = Some(14 * 3_600_000);
    let prediction = predictor.predict(&input).unwrap();
    assert_eq!(prediction.predicted_workload, WorkloadType::Gaming);
    assert_eq!(prediction.confidence, 0.75);
}

#[test]
fn defaults_are_sensible() {
    let p = ShortHorizonPredictor::with_defaults();
    assert_eq!(p.min_samples(), 3);
    assert_eq!(p.horizon_ms(), 5_000);
    assert!((p.min_confidence() - 0.5).abs() < f32::EPSILON);
}
