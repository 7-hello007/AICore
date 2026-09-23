use std::sync::Arc;

use aicore_contracts::{
    execution::{result::ExecutionResult, status::ExecutionStatus},
    policy::{priority::PolicyPriority, resource_policy::ResourcePolicy},
    traits::{
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
        power_controller::IPowerController,
        process_controller::IProcessController,
    },
};

/// Reads back only the platform controls that reported an actual successful write.
/// This checks applied state, not a claimed performance or energy improvement.
pub struct PlatformPostConditionChecker {
    process: Option<Arc<dyn IProcessController>>,
    power: Option<Arc<dyn IPowerController>>,
}

impl PlatformPostConditionChecker {
    pub fn new(
        process: Option<Arc<dyn IProcessController>>,
        power: Option<Arc<dyn IPowerController>>,
    ) -> Self {
        Self { process, power }
    }
}

impl IPostConditionChecker for PlatformPostConditionChecker {
    fn verify(&self, policy: &ResourcePolicy, result: &ExecutionResult) -> PostConditionResult {
        let mut checked = 0usize;
        let mut unverified = false;

        for outcome in result
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == ExecutionStatus::Success)
        {
            let observed = match outcome.resource.as_str() {
                "power.profile" => self.power.as_ref().map(|power| {
                    power
                        .get_profile()
                        .map(|actual| actual == policy.power.mode)
                }),
                "process.priority" | "process.throttle" => {
                    let expected = if outcome.resource == "process.throttle" {
                        PolicyPriority::Background
                    } else {
                        policy.cpu.priority
                    };
                    match (self.process.as_ref(), parse_pid(&outcome.target)) {
                        (Some(process), Some(pid)) => {
                            Some(process.get_priority(pid).map(|actual| actual == expected))
                        },
                        _ => None,
                    }
                },
                "process.affinity" => match (self.process.as_ref(), parse_pid(&outcome.target)) {
                    (Some(process), Some(pid)) => {
                        policy.cpu.affinity_cores.as_ref().map(|expected| {
                            process.get_affinity(pid).map(|actual| {
                                let mut actual = actual.cores;
                                let mut expected = expected.clone();
                                actual.sort_unstable();
                                expected.sort_unstable();
                                actual == expected
                            })
                        })
                    },
                    _ => None,
                },
                _ => None,
            };

            match observed {
                Some(Ok(true)) => checked += 1,
                Some(Ok(false)) => {
                    return PostConditionResult::Failed(format!(
                        "{} readback differs from policy at {}",
                        outcome.resource, outcome.target
                    ));
                },
                Some(Err(error)) => {
                    return PostConditionResult::Failed(format!(
                        "{} readback failed at {}: {}",
                        outcome.resource, outcome.target, error
                    ));
                },
                None => unverified = true,
            }
        }

        if checked > 0 && !unverified {
            PostConditionResult::Passed
        } else {
            PostConditionResult::NotApplicable
        }
    }

    fn checker_name(&self) -> &'static str {
        "platform_post_condition_checker"
    }
}

fn parse_pid(target: &str) -> Option<u32> {
    target.strip_prefix("pid=")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::{
        errors::ContractResult,
        execution::{result::ExecutionOutcome, status::ExecutionOverallStatus},
        policy::resource_policy::PowerMode,
        traits::process_controller::AffinityMask,
    };

    struct MockProcess {
        priority: PolicyPriority,
    }

    impl IProcessController for MockProcess {
        fn set_priority(&self, _pid: u32, _priority: PolicyPriority) -> ContractResult<()> {
            panic!("readback must not write")
        }

        fn get_priority(&self, _pid: u32) -> ContractResult<PolicyPriority> {
            Ok(self.priority)
        }

        fn get_affinity(&self, _pid: u32) -> ContractResult<AffinityMask> {
            Ok(AffinityMask::new(vec![2, 1]))
        }

        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }

        fn platform_name(&self) -> &'static str {
            "mock"
        }
    }

    struct MockPower {
        mode: PowerMode,
    }

    impl IPowerController for MockPower {
        fn set_profile(&self, _mode: PowerMode) -> ContractResult<PowerMode> {
            panic!("readback must not write")
        }

        fn get_profile(&self) -> ContractResult<PowerMode> {
            Ok(self.mode)
        }

        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }

        fn platform_name(&self) -> &'static str {
            "mock"
        }
    }

    fn result(resources: &[(&str, &str)]) -> ExecutionResult {
        ExecutionResult {
            policy_id: "test".to_string(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 1,
            outcomes: resources
                .iter()
                .map(|(resource, target)| ExecutionOutcome {
                    resource: (*resource).to_string(),
                    target: (*target).to_string(),
                    status: ExecutionStatus::Success,
                    message: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn matching_readback_passes_without_writes() {
        let process = Arc::new(MockProcess {
            priority: PolicyPriority::High,
        });
        let power = Arc::new(MockPower {
            mode: PowerMode::Balanced,
        });
        let checker = PlatformPostConditionChecker::new(Some(process), Some(power));
        let mut policy = ResourcePolicy::default();
        policy.cpu.priority = PolicyPriority::High;
        policy.cpu.affinity_cores = Some(vec![1, 2]);
        assert_eq!(
            checker.verify(
                &policy,
                &result(&[
                    ("power.profile", "system"),
                    ("process.priority", "pid=42"),
                    ("process.affinity", "pid=42"),
                ]),
            ),
            PostConditionResult::Passed
        );
    }

    #[test]
    fn mismatched_readback_is_failure() {
        let process = Arc::new(MockProcess {
            priority: PolicyPriority::Low,
        });
        let checker = PlatformPostConditionChecker::new(Some(process), None);
        let mut policy = ResourcePolicy::default();
        policy.cpu.priority = PolicyPriority::High;
        assert!(matches!(
            checker.verify(&policy, &result(&[("process.priority", "pid=42")])),
            PostConditionResult::Failed(_)
        ));
    }

    #[test]
    fn unknown_or_missing_readback_never_claims_success() {
        let checker = PlatformPostConditionChecker::new(None, None);
        let policy = ResourcePolicy::default();
        assert_eq!(
            checker.verify(&policy, &result(&[("power.profile", "system")])),
            PostConditionResult::NotApplicable
        );
        assert_eq!(
            checker.verify(&policy, &result(&[("gpu.power", "gpu0")])),
            PostConditionResult::NotApplicable
        );
    }
}
