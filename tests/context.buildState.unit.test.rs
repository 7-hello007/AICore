use std::sync::Arc;

use aicore_contracts::{
    context::workload::{InteractionLevel, WorkloadType},
    telemetry::{
        application::ApplicationSnapshot, process::ProcessSnapshot, snapshot::ComputerSnapshot,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    domain::computer_state::{ComputerState, SystemHealth},
};

fn make_state() -> ComputerState {
    let mut snapshot = ComputerSnapshot::default();
    snapshot.cpu.logical_cores = 8;
    snapshot.applications = vec![ApplicationSnapshot {
        name: "Code".to_string(),
        pid: 1000,
        is_foreground: true,
        window_title: Some("main.rs - aicore".to_string()),
    }];
    snapshot.top_processes = vec![ProcessSnapshot {
        pid: 1000,
        name: "code".to_string(),
        cpu_usage_percent: 12.0,
        memory_mb: 512,
        is_foreground: true,
    }];

    ComputerState {
        snapshot,
        health: SystemHealth {
            cpu_available: true,
            ..Default::default()
        },
    }
}

#[test]
fn engine_builds_development_context() {
    let engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let state = make_state();
    let ctx = engine.build(&state);

    assert_eq!(ctx.foreground_app.as_deref(), Some("Code"));
    assert_eq!(ctx.foreground_process_name.as_deref(), Some("code"));
    assert_eq!(
        ctx.foreground_window_title.as_deref(),
        Some("main.rs - aicore")
    );
    assert_eq!(ctx.workload_type, WorkloadType::Development);
    assert_eq!(ctx.interaction_level, InteractionLevel::Interactive);
    assert!(ctx.confidence > 0.0);
    assert!(ctx.timestamp_unix_ms.is_some());
}

#[test]
fn engine_marks_idle_when_no_apps() {
    let engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let state = ComputerState::default();
    let ctx = engine.build(&state);

    assert!(ctx.foreground_app.is_none());
    assert_eq!(ctx.workload_type, WorkloadType::Unknown);
    assert_eq!(ctx.interaction_level, InteractionLevel::Idle);
}

#[test]
fn engine_reports_classifier_name() {
    let engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    assert_eq!(engine.classifier_name(), "rule_based");
}
