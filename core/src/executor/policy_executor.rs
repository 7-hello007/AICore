use std::collections::BTreeMap;
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
        policy_executor::IPolicyExecutor,
        power_controller::IPowerController,
        process_controller::{IProcessController, ProcessPrioritySnapshot},
    },
};

/// Default policy executor.
///
/// Performs only low-risk controls:
/// 1. System power mode.
/// 2. Foreground-process priority.
/// 3. Background-process throttling (when requested by policy).
///
/// GPU/NPU actions belong to Runtime Router / Hardware Executor; CPU affinity to ProcessController.
pub struct DefaultPolicyExecutor {
    pub(super) process: Option<Arc<dyn IProcessController>>,
    pub(super) power: Option<Arc<dyn IPowerController>>,
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
        match self.begin_transaction(policy, targets) {
            Ok(Some(mut transaction)) => {
                let mut result = transaction.apply();
                if result.outcomes.iter().any(|o| o.status.is_real_failure()) {
                    result.outcomes.extend(transaction.restore());
                    // Restoration never turns the failed requested operation into success.
                    result.overall_status = ExecutionOverallStatus::AllFailed;
                }
                result
            },
            Ok(None) => unreachable!("default executor always captures a transaction"),
            Err(error) => capture_failed(policy, error),
        }
    }

    fn begin_transaction(
        &self,
        policy: &ResourcePolicy,
        targets: &ExecutionTargets,
    ) -> aicore_contracts::errors::ContractResult<
        Option<Box<dyn aicore_contracts::traits::policy_executor::IPolicyTransaction + '_>>,
    > {
        super::transaction::capture(self, policy, targets).map(Some)
    }

    fn name(&self) -> &'static str {
        "default_policy_executor"
    }
}

impl DefaultPolicyExecutor {
    pub(super) fn apply_captured(
        &self,
        policy: &ResourcePolicy,
        targets: &ExecutionTargets,
        originals: &BTreeMap<u32, ProcessPrioritySnapshot>,
        power_captured: bool,
    ) -> ExecutionResult {
        let started = now_unix_ms();
        let mut outcomes: Vec<ExecutionOutcome> = Vec::new();

        // 1. Power mode (system level)
        if targets.process_only {
            outcomes.push(ExecutionOutcome {
                resource: "power.profile".into(),
                target: "system".into(),
                status: ExecutionStatus::Skipped,
                message: "application preparation is process-only".into(),
            });
        } else if self.power.is_some() && !power_captured {
            outcomes.push(ExecutionOutcome {
                resource: "power.profile".into(),
                target: "system".into(),
                status: ExecutionStatus::NotSupported,
                message: "exact original power state unavailable; no power write attempted".into(),
            });
        } else if let Some(power) = &self.power {
            match power.set_profile(policy.power.mode) {
                Ok(applied) => outcomes.push(ExecutionOutcome {
                    resource: "power.profile".to_string(),
                    target: "system".to_string(),
                    status: if applied == policy.power.mode {
                        ExecutionStatus::Success
                    } else {
                        ExecutionStatus::Failed
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

        // 2. Foreground-process priority.
        if let Some(process) = &self.process {
            let fg_priority = policy.cpu.priority;
            for pid in &targets.foreground_pids {
                match original_for(originals, *pid)
                    .and_then(|saved| process.set_priority_captured(saved, fg_priority))
                {
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

            // 2.5 CPU affinity.
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
                        match original_for(originals, *pid)
                            .and_then(|saved| process.set_affinity_captured(saved, &mask))
                        {
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

            // 3. Background throttling.
            if policy.background.throttle {
                for pid in &targets.background_pids {
                    // Skip PIDs that also appear in the foreground set.
                    if targets.foreground_pids.contains(pid) {
                        continue;
                    }
                    match original_for(originals, *pid).and_then(|saved| {
                        process.set_priority_captured(saved, PolicyPriority::Background)
                    }) {
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
}

fn original_for(
    originals: &BTreeMap<u32, ProcessPrioritySnapshot>,
    pid: u32,
) -> Result<&ProcessPrioritySnapshot, ContractError> {
    originals
        .get(&pid)
        .ok_or_else(|| ContractError::ContractInvalid("process original identity missing".into()))
}

pub(super) fn capture_failed(policy: &ResourcePolicy, error: ContractError) -> ExecutionResult {
    let outcomes = vec![outcome_from_error(
        "execution.capture",
        "transaction",
        error,
    )];
    ExecutionResult {
        policy_id: policy.policy_id.clone(),
        overall_status: ExecutionOverallStatus::from_outcomes(&outcomes),
        started_at_unix_ms: now_unix_ms(),
        finished_at_unix_ms: now_unix_ms(),
        outcomes,
    }
}

pub(super) fn outcome_from_error(
    resource: &str,
    target: &str,
    err: ContractError,
) -> ExecutionOutcome {
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
