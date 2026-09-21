use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    errors::ContractError,
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::{priority::PolicyPriority, resource_policy::ResourcePolicy},
    traits::{
        policy_executor::IPolicyExecutor, power_controller::IPowerController,
        process_controller::IProcessController,
    },
};

/// 默认策略执行器。
///
/// 只做低风险控制：
/// 1. 系统电源模式。
/// 2. 前台进程优先级。
/// 3. 后台进程节流（若策略要求）。
///
/// GPU / NPU 由 Runtime Router / Hardware Executor 处理；CPU 亲和性由 ProcessController 处理。
pub struct DefaultPolicyExecutor {
    process: Option<Arc<dyn IProcessController>>,
    power: Option<Arc<dyn IPowerController>>,
}

impl DefaultPolicyExecutor {
    pub fn new(
        process: Option<Arc<dyn IProcessController>>,
        power: Option<Arc<dyn IPowerController>>,
    ) -> Self {
        Self { process, power }
    }
}

impl IPolicyExecutor for DefaultPolicyExecutor {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
        let started = now_unix_ms();
        let mut outcomes: Vec<ExecutionOutcome> = Vec::new();

        // 1. 电源模式（系统级）
        if let Some(power) = &self.power {
            match power.set_profile(policy.power.mode) {
                Ok(applied) => outcomes.push(ExecutionOutcome {
                    resource: "power.profile".to_string(),
                    target: "system".to_string(),
                    status: if applied == policy.power.mode {
                        ExecutionStatus::Success
                    } else {
                        ExecutionStatus::Skipped
                    },
                    message: format!("requested={:?} applied={:?}", policy.power.mode, applied),
                }),
                Err(e) => outcomes.push(outcome_from_error("power.profile", "system", e)),
            }
        } else {
            outcomes.push(ExecutionOutcome {
                resource: "power.profile".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::Skipped,
                message: "no IPowerController registered".to_string(),
            });
        }

        // 2. 前台进程优先级
        if let Some(process) = &self.process {
            let fg_priority = policy.cpu.priority;
            for pid in &targets.foreground_pids {
                match process.set_priority(*pid, fg_priority) {
                    Ok(()) => outcomes.push(ExecutionOutcome {
                        resource: "process.priority".to_string(),
                        target: format!("pid={}", pid),
                        status: ExecutionStatus::Success,
                        message: format!("set to {:?}", fg_priority),
                    }),
                    Err(e) => outcomes.push(outcome_from_error(
                        "process.priority",
                        &format!("pid={}", pid),
                        e,
                    )),
                }
            }

            // 2.5 CPU 亲和性
            if let Some(cores) = &policy.cpu.affinity_cores {
                if let Some(process) = &self.process {
                    let mask = aicore_contracts::traits::process_controller::AffinityMask::new(
                        cores.clone(),
                    );
                    for pid in targets
                        .foreground_pids
                        .iter()
                        .chain(targets.background_pids.iter())
                    {
                        match process.set_affinity(*pid, &mask) {
                            Ok(()) => outcomes.push(ExecutionOutcome {
                                resource: "process.affinity".to_string(),
                                target: format!("pid={}", pid),
                                status: ExecutionStatus::Success,
                                message: format!("set cores={:?}", cores),
                            }),
                            Err(e) => outcomes.push(outcome_from_error(
                                "process.affinity",
                                &format!("pid={}", pid),
                                e,
                            )),
                        }
                    }
                } else {
                    outcomes.push(ExecutionOutcome {
                        resource: "process.affinity".to_string(),
                        target: "*".to_string(),
                        status: ExecutionStatus::NotSupported,
                        message: "no IProcessController registered".to_string(),
                    });
                }
            }

            // 3. 后台节流
            if policy.background.throttle {
                for pid in &targets.background_pids {
                    // 跳过与前台重叠的 PID
                    if targets.foreground_pids.contains(pid) {
                        continue;
                    }
                    match process.set_priority(*pid, PolicyPriority::Background) {
                        Ok(()) => outcomes.push(ExecutionOutcome {
                            resource: "process.throttle".to_string(),
                            target: format!("pid={}", pid),
                            status: ExecutionStatus::Success,
                            message: "throttled to Background".to_string(),
                        }),
                        Err(e) => outcomes.push(outcome_from_error(
                            "process.throttle",
                            &format!("pid={}", pid),
                            e,
                        )),
                    }
                }
            }
        } else {
            outcomes.push(ExecutionOutcome {
                resource: "process.priority".to_string(),
                target: "*".to_string(),
                status: ExecutionStatus::Skipped,
                message: "no IProcessController registered".to_string(),
            });
        }

        let finished = now_unix_ms();
        let overall_status = ExecutionOverallStatus::from_outcomes(&outcomes);

        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status,
            started_at_unix_ms: started,
            finished_at_unix_ms: finished,
            outcomes,
        }
    }

    fn name(&self) -> &'static str {
        "default_policy_executor"
    }
}

fn outcome_from_error(resource: &str, target: &str, err: ContractError) -> ExecutionOutcome {
    let (status, message) = match err {
        ContractError::PermissionDenied(m) => (ExecutionStatus::PermissionDenied, m),
        ContractError::NotSupported(m) => (ExecutionStatus::NotSupported, m),
        ContractError::CapabilityUnavailable(m) => (ExecutionStatus::NotSupported, m),
        ContractError::ResourceUnavailable(m) => (ExecutionStatus::Failed, m),
        ContractError::AdapterUnavailable(m) => (ExecutionStatus::Failed, m),
        ContractError::Timeout(m) => (ExecutionStatus::Failed, m),
        ContractError::InvalidRequest(m) => (ExecutionStatus::Failed, m),
        ContractError::AiInferenceFailed(m) => (ExecutionStatus::Failed, m),
        ContractError::AiTimeout(m) => (ExecutionStatus::Failed, m),
        ContractError::AppNotRegistered(m) => (ExecutionStatus::Failed, m),
        ContractError::ContractInvalid(m) => (ExecutionStatus::Failed, m),
        ContractError::NotFound(m) => (ExecutionStatus::Failed, m),
        ContractError::AlreadyExists(m) => (ExecutionStatus::Failed, m),
        ContractError::ConfigInvalid(m) => (ExecutionStatus::Failed, m),
        ContractError::PluginInvalid(m) => (ExecutionStatus::Failed, m),
        ContractError::PluginApiVersionMismatch { plugin, core } => (
            ExecutionStatus::Failed,
            format!(
                "plugin api version mismatch: plugin={:?} core={:?}",
                plugin, core
            ),
        ),
        ContractError::PluginFailed(m) => (ExecutionStatus::Failed, m),
        ContractError::PluginSignatureInvalid(m) => (ExecutionStatus::Failed, m),
        ContractError::TierInvalid(m) => (ExecutionStatus::Failed, m),
        ContractError::TierLevelCannotControlNative(level) => (
            ExecutionStatus::Failed,
            format!("tier level cannot control native: {:?}", level),
        ),
        ContractError::IpcProtocolMismatch { got, expected } => (
            ExecutionStatus::Failed,
            format!("ipc protocol mismatch: got={} expected={}", got, expected),
        ),
        ContractError::LockPoisoned(m) => (ExecutionStatus::Failed, m),
        ContractError::Internal(m) => (ExecutionStatus::Failed, m),
    };
    ExecutionOutcome {
        resource: resource.to_string(),
        target: target.to_string(),
        status,
        message,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
