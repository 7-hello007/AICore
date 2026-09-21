use std::sync::Arc;

use aicore_contracts::{
    context::snapshot::ContextSnapshot,
    execution::{status::ExecutionOverallStatus, targets::ExecutionTargets},
    policy::{priority::PolicySource, resource_policy::ResourcePolicy},
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::RuleBasedPolicyEngine,
    prediction::ShortHorizonPredictor,
};

use crate::evidence::ScenarioEvidence;
use crate::scenarios::Scenario;

use aicore_contracts::traits::policy_engine::IPolicyEngine;
use aicore_contracts::traits::task_classifier::ITaskClassifier;

/// 执行一个场景，返回证据。
pub fn run_scenario(scenario: &Scenario) -> ScenarioEvidence {
    let mut failures: Vec<String> = Vec::new();

    // 1. 构造场景 snapshot
    let snapshot = (scenario.snapshot)();

    // 2. Context Engine 推断 workload
    let _ctx_engine = ContextEngine::new(Arc::new(RuleBasedClassifier::new()));
    let classifier = RuleBasedClassifier::new();
    let input = build_classification_input(&snapshot);
    let inferred = classifier.classify(&input);

    // 3. 构造 ContextSnapshot（模拟 Context Engine 输出）
    let context = ContextSnapshot {
        foreground_app: Some(scenario.foreground_app.to_string()),
        workload_type: inferred,
        confidence: if inferred == scenario.expected_workload {
            0.9
        } else {
            0.3
        },
        ..Default::default()
    };

    // 4. Policy Engine（规则 + 预测器可选）
    let predictor = Arc::new(ShortHorizonPredictor::with_defaults());
    let _ = predictor; // 预测器在无 memory 时返回 None；本场景不带 memory
    let policy_engine = RuleBasedPolicyEngine::new();
    let policy: ResourcePolicy = policy_engine.generate(&snapshot, &context);

    // 5. 检查
    if inferred != scenario.expected_workload {
        failures.push(format!(
            "classifier returned {:?}, expected {:?}",
            inferred, scenario.expected_workload
        ));
    }

    if policy.policy_id.trim().is_empty() {
        failures.push("policy_id is empty".to_string());
    }

    if policy.reason.trim().is_empty() {
        failures.push("policy reason is empty".to_string());
    }

    // 6. 模拟执行器输出（不依赖真实控制）
    let overall_status = ExecutionOverallStatus::NoOp;

    ScenarioEvidence {
        scenario: scenario.name.to_string(),
        platform: std::env::consts::OS.to_string(),
        app_version: scenario.app_version.to_string(),
        expected_workload: format!("{:?}", scenario.expected_workload),
        inferred_workload: format!("{:?}", inferred),
        classifier_confidence: context.confidence,
        policy_id: policy.policy_id.clone(),
        policy_source: format!("{:?}", policy.source),
        policy_reason: policy.reason.clone(),
        overall_status: format!("{:?}", overall_status),
        outcomes_count: 0,
        passed: failures.is_empty(),
        failures,
    }
}

fn build_classification_input(
    snapshot: &aicore_contracts::telemetry::ComputerSnapshot,
) -> aicore_contracts::traits::task_classifier::ClassificationInput {
    let fg_app = snapshot
        .applications
        .iter()
        .find(|a| a.is_foreground)
        .or_else(|| snapshot.applications.first())
        .cloned();
    let fg_proc = snapshot
        .top_processes
        .iter()
        .find(|p| p.is_foreground)
        .cloned();

    aicore_contracts::traits::task_classifier::ClassificationInput {
        cpu_usage_percent: snapshot.cpu.usage_percent,
        gpu_usage_percent: snapshot.gpu.usage_percent,
        npu_usage_percent: snapshot.npu.usage_percent,
        memory_usage_percent: snapshot.memory.usage_percent,
        foreground_app: fg_app.as_ref().map(|a| a.name.clone()),
        foreground_process: fg_proc.as_ref().map(|p| p.name.clone()),
        foreground_window: fg_app.as_ref().and_then(|a| a.window_title.clone()),
        active_process_names: snapshot
            .top_processes
            .iter()
            .map(|p| p.name.clone())
            .collect(),
    }
}

// 保持 import 使用
#[allow(dead_code)]
fn _unused(_t: ExecutionTargets, _s: PolicySource) {}
