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

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxPowerController as PlatformPowerController;
#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxProcessController as PlatformProcessController;

#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsPowerController as PlatformPowerController;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsProcessController as PlatformProcessController;

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
    let process = Arc::new(PlatformProcessController::new());
    let power = Arc::new(PlatformPowerController::new());

    let exec = DefaultPolicyExecutor::new(Some(process), Some(power));
    let policy = make_policy();
    let targets = ExecutionTargets::default();

    let result = exec.execute(&policy, &targets);

    assert_eq!(result.policy_id, "policy-test");
    assert!(!result.outcomes.is_empty());

    // 具体状态取决于环境能力：
    // - 若 power.profile 可用 → AllSucceeded 或 PartiallySucceeded
    // - 若 power.profile 不可用（NotSupported）→ NoOp
    // 唯一不能出现的是 AllFailed（因为没有真失败）。
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
