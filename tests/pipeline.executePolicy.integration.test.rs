use std::sync::Arc;

use aicore_contracts::{
    execution::{status::ExecutionOverallStatus, targets::ExecutionTargets},
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    traits::policy_executor::IPolicyExecutor,
};
use aicore_core::executor::DefaultPolicyExecutor;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "native.fixture.rs"]
mod native_fixture;
#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "native.power.fixture.rs"]
mod native_power_fixture;

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn make_policy() -> ResourcePolicy {
    ResourcePolicy {
        policy_id: "policy-test".to_string(),
        priority: PolicyPriority::Normal,
        source: PolicySource::RuleBased,
        reason: "test".to_string(),
        cpu: CpuPolicy {
            priority: PolicyPriority::Normal,
            affinity_cores: None,
        },
        gpu: GpuPolicy {
            priority: PolicyPriority::Normal,
            workload_preference: None,
        },
        npu: NpuPolicy {
            priority: PolicyPriority::Normal,
            preferred: false,
        },
        memory: MemoryPolicy {
            priority: PolicyPriority::Normal,
            max_usage_percent: None,
        },
        power: PowerPolicy {
            mode: PowerMode::Balanced,
        },
        background: BackgroundPolicy {
            throttle: false,
            max_cpu_percent: None,
        },
        ..Default::default()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn executor_runs_and_reports_outcomes() {
    let process = Arc::new(native_fixture::OwnedProcess::new());

    let power = native_power_fixture::OwnedPowerSnapshot::new()
        .ok()
        .map(Arc::new);
    if power.is_none() {
        eprintln!("NOT VERIFIED native power snapshot unavailable; no global mutation attempted");
    }
    let exec = DefaultPolicyExecutor::new(
        Some(process.clone()),
        power.map(|p| p as Arc<dyn aicore_contracts::traits::power_controller::IPowerController>),
    );
    let policy = make_policy();
    let targets = ExecutionTargets {
        process_only: false,
        ..process.targets()
    };

    let result = exec.execute(&policy, &targets);

    assert_eq!(result.policy_id, "policy-test");
    assert!(!result.outcomes.is_empty());

    // Native control targets only the owned child; global power is separately verified.
    assert_ne!(
        result.overall_status,
        ExecutionOverallStatus::AllFailed,
        "no real failure expected, got {:?} with outcomes {:?}",
        result.overall_status,
        result.outcomes
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn executor_without_controllers_is_noop() {
    let exec = DefaultPolicyExecutor::new(None, None);
    let policy = make_policy();
    let targets = ExecutionTargets::default();

    let result = exec.execute(&policy, &targets);
    assert!(!result.outcomes.is_empty());
    assert!(result
        .outcomes
        .iter()
        .all(|o| o.status != aicore_contracts::execution::status::ExecutionStatus::Success));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn executor_reports_name() {
    let exec = DefaultPolicyExecutor::new(None, None);
    assert_eq!(exec.name(), "default_policy_executor");
}
