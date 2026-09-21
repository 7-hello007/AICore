use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::PowerMode,
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::policy_engine::IPolicyEngine,
};
use aicore_core::policy::RuleBasedPolicyEngine;

fn ctx(workload: WorkloadType, interaction: InteractionLevel) -> ContextSnapshot {
    ContextSnapshot {
        workload_type: workload,
        interaction_level: interaction,
        confidence: 0.9,
        ..Default::default()
    }
}

#[test]
fn engine_generates_development_policy() {
    let engine = RuleBasedPolicyEngine::new();
    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::Development, InteractionLevel::Interactive),
    );

    assert!(!policy.policy_id.is_empty());
    assert_eq!(policy.source, PolicySource::RuleBased);
    assert!(policy.cpu.priority >= PolicyPriority::High);
    assert!(policy.background.throttle);
    assert!(policy.reason.contains("Development"));
}

#[test]
fn engine_generates_gaming_performance_policy() {
    let engine = RuleBasedPolicyEngine::new();
    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::Gaming, InteractionLevel::Interactive),
    );

    assert_eq!(policy.power.mode, PowerMode::Performance);
    assert_eq!(policy.gpu.priority, PolicyPriority::High);
}

#[test]
fn engine_downgrades_when_idle() {
    let engine = RuleBasedPolicyEngine::new();
    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::Development, InteractionLevel::Idle),
    );

    assert_eq!(policy.cpu.priority, PolicyPriority::Low);
    assert_eq!(policy.gpu.priority, PolicyPriority::Low);
    assert_eq!(policy.npu.priority, PolicyPriority::Low);
    assert_eq!(policy.memory.priority, PolicyPriority::Low);
}

#[test]
fn engine_prefers_power_save_on_battery() {
    let engine = RuleBasedPolicyEngine::new();
    let mut snapshot = ComputerSnapshot::default();
    snapshot.battery.present = true;
    snapshot.battery.charging = Some(false);
    snapshot.power.ac_connected = false;

    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::GeneralOffice, InteractionLevel::Interactive),
    );

    assert_eq!(policy.power.mode, PowerMode::PowerSave);
}

#[test]
fn engine_keeps_performance_for_heavy_on_battery() {
    let engine = RuleBasedPolicyEngine::new();
    let mut snapshot = ComputerSnapshot::default();
    snapshot.battery.present = true;
    snapshot.battery.charging = Some(false);
    snapshot.power.ac_connected = false;

    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::Gaming, InteractionLevel::Interactive),
    );

    assert_eq!(policy.power.mode, PowerMode::Balanced);
}

#[test]
fn engine_reports_name() {
    assert_eq!(
        RuleBasedPolicyEngine::new().name(),
        "rule_based_policy_engine"
    );
}

#[test]
fn engine_overall_priority_is_max_of_subresources() {
    let engine = RuleBasedPolicyEngine::new();
    let snapshot = ComputerSnapshot::default();
    let policy = engine.generate(
        &snapshot,
        &ctx(WorkloadType::AiInference, InteractionLevel::Interactive),
    );

    let max = [
        policy.cpu.priority,
        policy.gpu.priority,
        policy.npu.priority,
        policy.memory.priority,
    ]
    .into_iter()
    .max()
    .unwrap();
    assert_eq!(policy.priority, max);
}
