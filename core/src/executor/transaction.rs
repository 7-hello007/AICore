//! Original-state recovery for the default platform policy executor.
use std::collections::BTreeSet;
use std::sync::MutexGuard;

use super::{
    policy_executor::{capture_failed, outcome_from_error, DefaultPolicyExecutor},
    PlatformPostConditionChecker,
};
use aicore_contracts::{
    errors::{ContractError, ContractResult},
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    traits::{
        policy_executor::IPolicyTransaction,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
        power_controller::PowerStateSnapshot,
        process_controller::{AffinityMask, ProcessPrioritySnapshot},
    },
};

struct ProcessOriginal {
    pid: u32,
    priority: Option<ProcessPrioritySnapshot>,
    affinity: Option<AffinityMask>,
}
struct Transaction<'a> {
    executor: &'a DefaultPolicyExecutor,
    _guard: MutexGuard<'a, ()>,
    policy: ResourcePolicy,
    targets: ExecutionTargets,
    processes: Vec<ProcessOriginal>,
    power: Option<PowerStateSnapshot>,
    attempted: BTreeSet<(String, String)>,
    applied: bool,
    restored: bool,
}

pub(super) fn capture<'a>(
    executor: &'a DefaultPolicyExecutor,
    policy: &ResourcePolicy,
    targets: &ExecutionTargets,
) -> ContractResult<Box<dyn IPolicyTransaction + 'a>> {
    let guard = aicore_contracts::traits::power_controller::control_transaction_lock()?;
    let mut processes = Vec::new();
    if let Some(process) = &executor.process {
        let ids: BTreeSet<_> = targets
            .foreground_pids
            .iter()
            .chain(targets.background_pids.iter())
            .copied()
            .collect();
        for pid in ids {
            let changes_priority =
                targets.foreground_pids.contains(&pid) || policy.background.throttle;
            if !changes_priority && policy.cpu.affinity_cores.is_none() {
                continue;
            }
            processes.push(ProcessOriginal {
                pid,
                priority: Some(process.capture_priority(pid)?),
                affinity: if policy.cpu.affinity_cores.is_some() {
                    Some(process.get_affinity(pid)?)
                } else {
                    None
                },
            });
        }
    }
    let power = if !targets.process_only {
        match &executor.power {
            Some(power) if power.supports_exact_state() => match power.capture_state() {
                Ok(saved) => Some(saved),
                Err(ContractError::NotSupported(_) | ContractError::CapabilityUnavailable(_)) => {
                    None
                },
                Err(error) => return Err(error),
            },
            _ => None,
        }
    } else {
        None
    };
    Ok(Box::new(Transaction {
        executor,
        _guard: guard,
        policy: policy.clone(),
        targets: targets.clone(),
        processes,
        power,
        attempted: BTreeSet::new(),
        applied: false,
        restored: false,
    }))
}

impl IPolicyTransaction for Transaction<'_> {
    fn apply(&mut self) -> ExecutionResult {
        if self.applied {
            return capture_failed(
                &self.policy,
                ContractError::InvalidRequest("transaction already applied".into()),
            );
        }
        self.applied = true;
        let originals = self
            .processes
            .iter()
            .filter_map(|saved| {
                saved
                    .priority
                    .as_ref()
                    .map(|priority| (saved.pid, priority.clone()))
            })
            .collect();
        let mut result = self.executor.apply_captured(
            &self.policy,
            &self.targets,
            &originals,
            self.power.is_some(),
        );
        // A setter may mutate and then fail. Save attempted failed operations too.
        self.attempted = result
            .outcomes
            .iter()
            .filter(|o| o.status == ExecutionStatus::Success || o.status.is_real_failure())
            .map(|o| (o.resource.clone(), o.target.clone()))
            .collect();
        let checker = PlatformPostConditionChecker::new(
            self.executor.process.clone(),
            self.executor.power.clone(),
        );
        if let PostConditionResult::Failed(reason) = checker.verify(&self.policy, &result) {
            result.outcomes.push(outcome_from_error(
                "execution.readback",
                "transaction",
                ContractError::AdapterUnavailable(reason),
            ));
            result.overall_status = ExecutionOverallStatus::from_outcomes(&result.outcomes);
        }
        result
    }

    fn restore(&mut self) -> Vec<ExecutionOutcome> {
        if !self.applied || self.restored {
            return vec![outcome_from_error(
                "execution.restore",
                "transaction",
                ContractError::InvalidRequest("transaction not applied or already restored".into()),
            )];
        }
        self.restored = true;
        let mut outcomes = Vec::new();
        if let Some(process) = &self.executor.process {
            // Reverse application order, but do not stop after one failure.
            for saved in self.processes.iter().rev() {
                let target = format!("pid={}", saved.pid);
                if let Some(mask) = &saved.affinity {
                    if self
                        .attempted
                        .contains(&("process.affinity".into(), target.clone()))
                    {
                        let result = saved
                            .priority
                            .as_ref()
                            .ok_or_else(|| {
                                ContractError::ContractInvalid(
                                    "process original identity missing".into(),
                                )
                            })
                            .and_then(|identity| process.restore_affinity(identity, mask))
                            .and_then(|()| {
                                let mut observed = process.get_affinity(saved.pid)?.cores;
                                let mut expected = mask.cores.clone();
                                observed.sort_unstable();
                                expected.sort_unstable();
                                if observed == expected {
                                    Ok(())
                                } else {
                                    Err(ContractError::AdapterUnavailable(
                                        "affinity restoration readback differs".into(),
                                    ))
                                }
                            });
                        outcomes.push(restoration("process.affinity", &target, result));
                    }
                }
                if let Some(priority) = &saved.priority {
                    if self
                        .attempted
                        .contains(&("process.priority".into(), target.clone()))
                        || self
                            .attempted
                            .contains(&("process.throttle".into(), target.clone()))
                    {
                        let result = process.restore_priority(priority).and_then(|()| {
                            if process.capture_priority(saved.pid)? == *priority {
                                Ok(())
                            } else {
                                Err(ContractError::AdapterUnavailable(
                                    "priority restoration readback differs".into(),
                                ))
                            }
                        });
                        outcomes.push(restoration("process.priority", &target, result));
                    }
                }
            }
        }
        if let (Some(power), Some(saved)) = (&self.executor.power, &self.power) {
            if self
                .attempted
                .contains(&("power.profile".into(), "system".into()))
            {
                let result = power.restore_state(saved).and_then(|()| {
                    if power.capture_state()? == *saved {
                        Ok(())
                    } else {
                        Err(ContractError::AdapterUnavailable(
                            "power restoration readback differs".into(),
                        ))
                    }
                });
                outcomes.push(restoration("power.profile", "system", result));
            }
        }
        // Verify the entire saved state after ALL restores, even if a setter
        // returned an error. Coupled controls can invalidate earlier readbacks.
        if let Some(process) = &self.executor.process {
            for saved in &self.processes {
                let target = format!("pid={}", saved.pid);
                if let Some(expected) = &saved.priority {
                    let readback = process.capture_priority(saved.pid).and_then(|actual| {
                        if actual == *expected {
                            Ok(())
                        } else {
                            Err(ContractError::AdapterUnavailable(
                                "final original priority/identity differs".into(),
                            ))
                        }
                    });
                    outcomes.push(restoration(
                        "process.priority.final_readback",
                        &target,
                        readback,
                    ));
                }
                if let Some(expected) = &saved.affinity {
                    let readback = process.get_affinity(saved.pid).and_then(|actual| {
                        let mut actual = actual.cores;
                        let mut expected = expected.cores.clone();
                        actual.sort_unstable();
                        expected.sort_unstable();
                        if actual == expected {
                            Ok(())
                        } else {
                            Err(ContractError::AdapterUnavailable(
                                "final original affinity differs".into(),
                            ))
                        }
                    });
                    outcomes.push(restoration(
                        "process.affinity.final_readback",
                        &target,
                        readback,
                    ));
                }
            }
        }
        if let (Some(power), Some(expected)) = (&self.executor.power, &self.power) {
            let readback = power.capture_state().and_then(|actual| {
                if actual == *expected {
                    Ok(())
                } else {
                    Err(ContractError::AdapterUnavailable(
                        "final original power state differs".into(),
                    ))
                }
            });
            outcomes.push(restoration(
                "power.profile.final_readback",
                "system",
                readback,
            ));
        }
        outcomes
    }
}

fn restoration(resource: &str, target: &str, result: ContractResult<()>) -> ExecutionOutcome {
    match result {
        Ok(()) => ExecutionOutcome {
            resource: resource.into(),
            target: target.into(),
            status: ExecutionStatus::Success,
            message: "original state independently verified".into(),
        },
        Err(error) => outcome_from_error(resource, target, error),
    }
}
