//! TransparentExecutor six-part test.

use std::sync::Arc;

use aicore_contracts::{
    control::stage::{ControlStage, StageStatus},
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    traits::{
        policy_executor::IPolicyExecutor,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
        transparent_executor::ITransparentExecutor,
    },
};
use aicore_core::executor::TransparentExecutor;

/// Test executor with a configurable return state.
struct MockExecutor {
    status: ExecutionOverallStatus,
}

impl IPolicyExecutor for MockExecutor {
    fn execute(&self, policy: &ResourcePolicy, _targets: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status: self.status,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "mock".to_string(),
                target: "system".to_string(),
                status: match self.status {
                    ExecutionOverallStatus::AllSucceeded => ExecutionStatus::Success,
                    ExecutionOverallStatus::AllFailed => ExecutionStatus::Failed,
                    _ => ExecutionStatus::Skipped,
                },
                message: "mock".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "mock_executor"
    }
}

/// Configure checker.
struct MockChecker {
    result: PostConditionResult,
}

impl IPostConditionChecker for MockChecker {
    fn verify(&self, _p: &ResourcePolicy, _r: &ExecutionResult) -> PostConditionResult {
        self.result.clone()
    }
    fn checker_name(&self) -> &'static str {
        "mock_checker"
    }
}

fn policy(id: &str) -> ResourcePolicy {
    ResourcePolicy {
        policy_id: id.to_string(),
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

#[test]
fn success_path_produces_six_stages() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("p1"), &ExecutionTargets::default(), None);

    // 6 Phase: Observ / Decide / Validate / Apply / Verify / Rollback
    assert_eq!(trace.stages.len(), 6);
    assert_eq!(
        trace.stage(ControlStage::Observe).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Decide).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Validate).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Apply).unwrap().status,
        StageStatus::Success
    );
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
    assert_eq!(
        trace.stage(ControlStage::Rollback).unwrap().status,
        StageStatus::Skipped
    );

    assert!(!trace.rollback.triggered);
    assert!(trace.is_success());
    assert_eq!(tx.name(), "transparent_executor");
}

#[test]
fn apply_failure_triggers_rollback_with_prior() {
    // A prior policy is advisory history, never a replacement for original OS state.
    p0_recovery::assert_original_recovery_with_prior();
}

#[test]
fn apply_failure_without_prior_records_failed_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllFailed,
    });
    let tx = TransparentExecutor::new(inner);

    let trace = tx.execute_transparent(&policy("new"), &ExecutionTargets::default(), None);

    assert!(trace.rollback.triggered);
    assert_eq!(trace.rollback.succeeded, Some(false));
    assert!(trace.rollback.to_policy_id.is_none());
    assert_eq!(
        trace.stage(ControlStage::Rollback).unwrap().status,
        StageStatus::Failed
    );
}

#[test]
fn noop_does_not_trigger_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::NoOp,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("p"), &ExecutionTargets::default(), None);

    assert!(!trace.rollback.triggered);
    assert!(!trace.is_success());
    // NoOp → Verify Not applicable
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
}

#[test]
fn checker_failure_triggers_rollback() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let checker = Arc::new(MockChecker {
        result: PostConditionResult::Failed("state mismatch".to_string()),
    });
    let tx = TransparentExecutor::new(inner).with_checker(checker);
    assert!(tx.has_checker());

    let prior = policy("prior");
    let trace = tx.execute_transparent(&policy("new"), &ExecutionTargets::default(), Some(&prior));

    assert!(trace.rollback.triggered);
    let verify = trace.stage(ControlStage::Verify).unwrap();
    assert_eq!(verify.status, StageStatus::Failed);
    assert!(verify.detail.contains("state mismatch"));
}

#[test]
fn checker_not_applicable_is_not_failure() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let checker = Arc::new(MockChecker {
        result: PostConditionResult::NotApplicable,
    });
    let tx = TransparentExecutor::new(inner).with_checker(checker);
    let trace = tx.execute_transparent(&policy("p"), &ExecutionTargets::default(), None);

    assert!(!trace.rollback.triggered);
    assert_eq!(
        trace.stage(ControlStage::Verify).unwrap().status,
        StageStatus::Skipped
    );
}

#[test]
fn trace_carries_policy_id_and_duration() {
    let inner = Arc::new(MockExecutor {
        status: ExecutionOverallStatus::AllSucceeded,
    });
    let tx = TransparentExecutor::new(inner);
    let trace = tx.execute_transparent(&policy("my-policy"), &ExecutionTargets::default(), None);

    assert_eq!(trace.policy_id, "my-policy");
    assert!(trace.trace_id.contains("my-policy"));
    // duration >= 0
    assert!(trace.total_duration_ms < 10_000);
}

// P0 recovery regressions use real executors with an in-memory OS boundary.
mod p0_recovery {
    use super::*;
    use aicore_contracts::{
        errors::{ContractError, ContractResult},
        traits::process_controller::{AffinityMask, IProcessController},
    };
    use aicore_core::executor::{DefaultPolicyExecutor, PlatformPostConditionChecker};
    use std::collections::BTreeMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    struct Process {
        values: Mutex<BTreeMap<u32, (PolicyPriority, Vec<u32>)>>,
        fail_apply: Mutex<bool>,
        restore_fault: u8,
        unreadable: bool,
        writes: AtomicUsize,
        captured_writes: AtomicUsize,
    }
    impl Process {
        fn new(restore_fault: u8, unreadable: bool) -> Self {
            Self {
                values: Mutex::new(BTreeMap::from([
                    (42, (PolicyPriority::Low, vec![0, 1, 2])),
                    (43, (PolicyPriority::Normal, vec![0, 2])),
                ])),
                fail_apply: Mutex::new(true),
                restore_fault,
                unreadable,
                writes: AtomicUsize::new(0),
                captured_writes: AtomicUsize::new(0),
            }
        }
    }
    impl IProcessController for Process {
        fn set_priority_captured(
            &self,
            saved: &aicore_contracts::traits::process_controller::ProcessPrioritySnapshot,
            value: PolicyPriority,
        ) -> ContractResult<()> {
            let original = if saved.pid == 42 {
                PolicyPriority::Low
            } else {
                PolicyPriority::Normal
            };
            assert_eq!(
                saved.state_id,
                serde_json::to_string(&original).unwrap(),
                "apply must carry original identity, not a new snapshot after mutation"
            );
            self.captured_writes.fetch_add(1, Ordering::SeqCst);
            self.set_priority(saved.pid, value)
        }
        fn set_affinity_captured(
            &self,
            saved: &aicore_contracts::traits::process_controller::ProcessPrioritySnapshot,
            value: &AffinityMask,
        ) -> ContractResult<()> {
            let original = if saved.pid == 42 {
                PolicyPriority::Low
            } else {
                PolicyPriority::Normal
            };
            assert_eq!(saved.state_id, serde_json::to_string(&original).unwrap());
            self.captured_writes.fetch_add(1, Ordering::SeqCst);
            self.set_affinity(saved.pid, value)
        }
        fn set_priority(&self, pid: u32, value: PolicyPriority) -> ContractResult<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if pid == 42 && value == PolicyPriority::Low {
                if self.restore_fault == 1 {
                    return Err(ContractError::AdapterUnavailable("restore denied".into()));
                }
                if self.restore_fault == 2 {
                    return Ok(());
                }
            }
            self.values.lock().unwrap().get_mut(&pid).unwrap().0 = value;
            if self.restore_fault == 3 && pid == 42 && value == PolicyPriority::Low {
                self.values.lock().unwrap().get_mut(&43).unwrap().0 = PolicyPriority::High;
            }
            let mut fail = self.fail_apply.lock().unwrap();
            if pid == 43 && *fail {
                *fail = false;
                return Err(ContractError::AdapterUnavailable(
                    "failed after write".into(),
                ));
            }
            Ok(())
        }
        fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority> {
            if self.unreadable && pid == 43 {
                return Err(ContractError::AdapterUnavailable(
                    "cannot capture original".into(),
                ));
            }
            Ok(self.values.lock().unwrap()[&pid].0)
        }
        fn set_affinity(&self, pid: u32, value: &AffinityMask) -> ContractResult<()> {
            self.values.lock().unwrap().get_mut(&pid).unwrap().1 = value.cores.clone();
            Ok(())
        }
        fn get_affinity(&self, pid: u32) -> ContractResult<AffinityMask> {
            Ok(AffinityMask::new(
                self.values.lock().unwrap()[&pid].1.clone(),
            ))
        }
        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }
        fn platform_name(&self) -> &'static str {
            "p0-test"
        }
    }
    fn setup(
        fault: u8,
        unreadable: bool,
    ) -> (
        Arc<Process>,
        TransparentExecutor,
        ResourcePolicy,
        ExecutionTargets,
    ) {
        let process = Arc::new(Process::new(fault, unreadable));
        let inner = Arc::new(DefaultPolicyExecutor::new(Some(process.clone()), None));
        let tx = TransparentExecutor::new(inner).with_checker(Arc::new(
            PlatformPostConditionChecker::new(Some(process.clone()), None),
        ));
        let mut p = policy("p0-change");
        p.cpu.priority = PolicyPriority::Interactive;
        p.cpu.affinity_cores = Some(vec![2]);
        let targets = ExecutionTargets {
            process_only: true,
            foreground_pids: vec![42, 43],
            background_pids: vec![],
        };
        (process, tx, p, targets)
    }
    pub(super) fn assert_original_recovery_with_prior() {
        let (process, tx, p, t) = setup(0, false);
        let mut prior = policy("prior");
        prior.cpu.priority = PolicyPriority::Background;
        let trace = tx.execute_transparent(&p, &t, Some(&prior));
        assert!(trace.rollback.triggered);
        assert_eq!(trace.rollback.succeeded, Some(true));
        assert_eq!(trace.rollback.to_policy_id, None);
        assert_eq!(
            trace.stage(ControlStage::Rollback).unwrap().status,
            StageStatus::Success
        );
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(
            process.values.lock().unwrap()[&42],
            (PolicyPriority::Low, vec![0, 1, 2])
        );
        assert_eq!(
            process.values.lock().unwrap()[&43],
            (PolicyPriority::Normal, vec![0, 2])
        );
    }
    #[test]
    fn p0_transactions_on_shared_controllers_cannot_interleave_snapshots() {
        use std::sync::mpsc;
        use std::time::Duration;
        let process = Arc::new(Process::new(0, false));
        let first = DefaultPolicyExecutor::new(Some(process.clone()), None);
        let second = DefaultPolicyExecutor::new(Some(process), None);
        let targets = ExecutionTargets {
            process_only: true,
            foreground_pids: vec![42],
            background_pids: vec![],
        };
        let p = policy("concurrent");
        let snapshot = first.begin_transaction(&p, &targets).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (captured_tx, captured_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let next = second.begin_transaction(&p, &targets).unwrap();
            captured_tx.send(()).unwrap();
            drop(next);
        });
        started_rx.recv().unwrap();
        let interleaved = captured_rx.recv_timeout(Duration::from_millis(80)).is_ok();
        drop(snapshot);
        worker.join().unwrap();
        assert!(
            !interleaved,
            "shared OS resources require serialization across executor wrappers"
        );
    }
    #[test]
    fn p0_partial_apply_restores_original_values_without_prior_policy() {
        let (process, tx, p, t) = setup(0, false);
        let trace = tx.execute_transparent(&p, &t, None);
        assert!(trace.rollback.triggered, "partial writes require rollback");
        assert_eq!(trace.rollback.succeeded, Some(true));
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(
            process.values.lock().unwrap()[&42],
            (PolicyPriority::Low, vec![0, 1, 2])
        );
        assert_eq!(
            process.values.lock().unwrap()[&43],
            (PolicyPriority::Normal, vec![0, 2])
        );
    }
    #[test]
    fn p0_partial_restore_failure_is_not_success_and_other_resources_restore() {
        let (process, tx, p, t) = setup(1, false);
        let trace = tx.execute_transparent(&p, &t, None);
        assert_eq!(trace.rollback.succeeded, Some(false));
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(
            process.values.lock().unwrap()[&43].0,
            PolicyPriority::Normal
        );
        assert_eq!(process.values.lock().unwrap()[&42].1, vec![0, 1, 2]);
    }
    #[test]
    fn p0_restore_ack_without_independent_readback_is_failure() {
        let (process, tx, p, t) = setup(2, false);
        let trace = tx.execute_transparent(&p, &t, None);
        assert_eq!(trace.rollback.succeeded, Some(false));
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(
            process.values.lock().unwrap()[&43].0,
            PolicyPriority::Normal
        );
    }
    #[test]
    fn p0_final_restore_readback_detects_coupled_resource_changes() {
        let (process, tx, p, t) = setup(3, false);
        let trace = tx.execute_transparent(&p, &t, None);
        assert_eq!(
            trace.rollback.succeeded,
            Some(false),
            "later restores can invalidate earlier readbacks"
        );
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(process.values.lock().unwrap()[&43].0, PolicyPriority::High);
    }
    #[test]
    fn p0_unreadable_original_rejects_all_writes_before_apply() {
        let (process, tx, p, t) = setup(0, true);
        let trace = tx.execute_transparent(&p, &t, None);
        assert_eq!(process.writes.load(Ordering::SeqCst), 0);
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
    }
    #[test]
    fn p0_normal_verified_apply_does_not_restore() {
        let (process, tx, p, mut t) = setup(0, false);
        t.foreground_pids = vec![42];
        let trace = tx.execute_transparent(&p, &t, None);
        assert!(!trace.rollback.triggered);
        assert!(trace.is_success());
        assert_eq!(
            process.values.lock().unwrap()[&42],
            (PolicyPriority::Interactive, vec![2])
        );
        assert_eq!(process.captured_writes.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn p0_partial_legacy_policy_replay_never_counts_as_restoration() {
        struct Legacy(AtomicUsize);
        impl IPolicyExecutor for Legacy {
            fn execute(&self, p: &ResourcePolicy, _: &ExecutionTargets) -> ExecutionResult {
                let first = self.0.fetch_add(1, Ordering::SeqCst) == 0;
                ExecutionResult {
                    policy_id: p.policy_id.clone(),
                    overall_status: if first {
                        ExecutionOverallStatus::AllFailed
                    } else {
                        ExecutionOverallStatus::PartiallySucceeded
                    },
                    started_at_unix_ms: 0,
                    finished_at_unix_ms: 0,
                    outcomes: vec![],
                }
            }
            fn name(&self) -> &'static str {
                "legacy"
            }
        }
        let tx = TransparentExecutor::new(Arc::new(Legacy(AtomicUsize::new(0))));
        let trace = tx.execute_transparent(
            &policy("new"),
            &ExecutionTargets::default(),
            Some(&policy("prior")),
        );
        assert_eq!(trace.rollback.succeeded, Some(false));
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
    }
    struct ExactPower {
        state: Mutex<String>,
        ack_only_restore: bool,
    }
    impl aicore_contracts::traits::power_controller::IPowerController for ExactPower {
        fn supports_exact_state(&self) -> bool {
            true
        }
        fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
            *self.state.lock().unwrap() = format!("{mode:?}");
            Ok(mode)
        }
        fn get_profile(&self) -> ContractResult<PowerMode> {
            Ok(if *self.state.lock().unwrap() == "Performance" {
                PowerMode::Performance
            } else {
                PowerMode::Balanced
            })
        }
        fn capture_state(
            &self,
        ) -> ContractResult<aicore_contracts::traits::power_controller::PowerStateSnapshot>
        {
            Ok(
                aicore_contracts::traits::power_controller::PowerStateSnapshot {
                    platform: "p0-power".into(),
                    state_id: self.state.lock().unwrap().clone(),
                },
            )
        }
        fn restore_state(
            &self,
            saved: &aicore_contracts::traits::power_controller::PowerStateSnapshot,
        ) -> ContractResult<()> {
            if !self.ack_only_restore {
                *self.state.lock().unwrap() = saved.state_id.clone();
            }
            Ok(())
        }
        fn capabilities(&self) -> Vec<&'static str> {
            vec![]
        }
        fn platform_name(&self) -> &'static str {
            "p0-power"
        }
    }
    #[test]
    fn p0_partial_cpu_failure_restores_exact_custom_power_state() {
        let process = Arc::new(Process::new(0, false));
        let power = Arc::new(ExactPower {
            state: Mutex::new("custom-guid-and-governors".into()),
            ack_only_restore: false,
        });
        let executor = Arc::new(DefaultPolicyExecutor::new(
            Some(process),
            Some(power.clone()),
        ));
        let tx = TransparentExecutor::new(executor);
        let mut p = policy("power-restore");
        p.power.mode = PowerMode::Performance;
        let targets = ExecutionTargets {
            process_only: false,
            foreground_pids: vec![43],
            background_pids: vec![],
        };
        let trace = tx.execute_transparent(&p, &targets, None);
        assert_eq!(trace.rollback.succeeded, Some(true));
        assert_eq!(*power.state.lock().unwrap(), "custom-guid-and-governors");
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
    }
    #[test]
    fn p0_power_restore_ack_is_not_verified_restoration() {
        let process = Arc::new(Process::new(0, false));
        let power = Arc::new(ExactPower {
            state: Mutex::new("custom-guid".into()),
            ack_only_restore: true,
        });
        let executor = Arc::new(DefaultPolicyExecutor::new(
            Some(process.clone()),
            Some(power.clone()),
        ));
        let tx = TransparentExecutor::new(executor);
        let mut p = policy("power-ack");
        p.power.mode = PowerMode::Performance;
        let targets = ExecutionTargets {
            process_only: false,
            foreground_pids: vec![43],
            background_pids: vec![],
        };
        let trace = tx.execute_transparent(&p, &targets, None);
        assert_eq!(trace.rollback.succeeded, Some(false));
        assert_eq!(trace.final_status, ExecutionOverallStatus::AllFailed);
        assert_eq!(
            process.values.lock().unwrap()[&43].0,
            PolicyPriority::Normal
        );
        assert_eq!(*power.state.lock().unwrap(), "Performance");
    }
    #[test]
    fn p0_legacy_coarse_power_snapshot_cannot_authorize_native_write() {
        use aicore_contracts::traits::power_controller::IPowerController;
        struct LegacyPower(ExactPower);
        impl IPowerController for LegacyPower {
            fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
                self.0.set_profile(mode)
            }
            fn get_profile(&self) -> ContractResult<PowerMode> {
                self.0.get_profile()
            }
            fn capabilities(&self) -> Vec<&'static str> {
                vec![]
            }
            fn platform_name(&self) -> &'static str {
                "legacy-native"
            }
        }
        let power = Arc::new(LegacyPower(ExactPower {
            state: Mutex::new("custom-native-guid".into()),
            ack_only_restore: false,
        }));
        let executor = DefaultPolicyExecutor::new(None, Some(power.clone()));
        let mut p = policy("legacy-power");
        p.power.mode = PowerMode::Performance;
        let result = executor.execute(&p, &ExecutionTargets::default());
        assert_eq!(*power.0.state.lock().unwrap(), "custom-native-guid");
        assert!(result
            .outcomes
            .iter()
            .any(|o| o.resource == "power.profile" && o.status == ExecutionStatus::NotSupported));
    }
}
