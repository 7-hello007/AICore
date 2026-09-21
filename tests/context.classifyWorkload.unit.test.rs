use aicore_contracts::{
    context::workload::WorkloadType,
    traits::task_classifier::{ClassificationInput, ITaskClassifier},
};
use aicore_core::context::RuleBasedClassifier;

fn input_with_app(app: &str) -> ClassificationInput {
    ClassificationInput {
        foreground_app: Some(app.to_string()),
        ..Default::default()
    }
}

#[test]
fn classifier_detects_development() {
    let c = RuleBasedClassifier::new();
    assert_eq!(
        c.classify(&input_with_app("Code")),
        WorkloadType::Development
    );
    assert_eq!(
        c.classify(&input_with_app("neovim")),
        WorkloadType::Development
    );
    assert_eq!(
        c.classify(&input_with_app("tmux")),
        WorkloadType::Development
    );
}

#[test]
fn classifier_detects_ai_inference() {
    let c = RuleBasedClassifier::new();
    assert_eq!(
        c.classify(&input_with_app("ollama")),
        WorkloadType::AiInference
    );
    assert_eq!(
        c.classify(&input_with_app("llama.cpp")),
        WorkloadType::AiInference
    );
}

#[test]
fn classifier_detects_video_conference() {
    let c = RuleBasedClassifier::new();
    assert_eq!(
        c.classify(&input_with_app("zoom")),
        WorkloadType::VideoConference
    );
}

#[test]
fn classifier_falls_back_to_unknown() {
    let c = RuleBasedClassifier::new();
    let input = ClassificationInput::default();
    assert_eq!(c.classify(&input), WorkloadType::Unknown);
}

#[test]
fn classifier_uses_npu_heuristic() {
    let c = RuleBasedClassifier::new();
    let input = ClassificationInput {
        npu_usage_percent: Some(60.0),
        ..Default::default()
    };
    assert_eq!(c.classify(&input), WorkloadType::AiInference);
}
