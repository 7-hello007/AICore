//! Explicit native control probe; never part of an automatic unit test.
//! Targets only its own freshly spawned child and restores priority before exit.
#[cfg(target_os = "windows")]
mod probe {
    use aicore_adapter_windows::{WindowsProcessController, WindowsTelemetryProvider};
    use aicore_contracts::{
        context::{snapshot::ContextSnapshot, workload::WorkloadType},
        effect::{EffectComparison, OverheadBudget},
        execution::{status::ExecutionStatus, targets::ExecutionTargets},
        policy::{
            priority::PolicyPriority,
            resource_policy::{CpuPolicy, ResourcePolicy},
        },
        telemetry::snapshot::ComputerSnapshot,
        traits::{
            policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
            process_controller::IProcessController, telemetry_provider::ITelemetryProvider,
        },
    };
    use aicore_core::{
        effect::observe_workload, executor::policy_executor::DefaultPolicyExecutor,
        policy::FeedbackPolicyEngine,
    };
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Child, Command, Stdio},
        sync::Arc,
    };
    struct OwnedChild {
        child: Child,
        controller: Arc<WindowsProcessController>,
        original: PolicyPriority,
    }
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let restoration = self
                .controller
                .set_priority(self.child.id(), self.original)
                .and_then(|()| self.controller.get_priority(self.child.id()));
            eprintln!(
                "cleanup pid={} restoration={restoration:?}",
                self.child.id()
            );
            if let Err(error) = self.child.kill() {
                eprintln!("child termination: {error}");
            }
            if let Err(error) = self.child.wait() {
                eprintln!("child reap: {error}");
            }
        }
    }
    struct Engine(PolicyPriority);
    impl IPolicyEngine for Engine {
        fn generate(&self, _: &ComputerSnapshot, _: &ContextSnapshot) -> ResourcePolicy {
            ResourcePolicy {
                policy_id: "owned-child-probe".into(),
                priority: self.0,
                cpu: CpuPolicy {
                    priority: self.0,
                    affinity_cores: None,
                },
                ..Default::default()
            }
        }
        fn name(&self) -> &'static str {
            "owned_child_probe"
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        if std::env::args().any(|a| a == "--child") {
            for line in std::io::stdin().lock().lines() {
                if line? != "run" {
                    break;
                }
                let mut hash = 1u64;
                for i in 0..1_000_000u64 {
                    hash = std::hint::black_box(
                        hash.wrapping_mul(6364136223846793005).wrapping_add(i),
                    );
                }
                println!("{hash}");
                std::io::stdout().flush()?;
            }
            return Ok(());
        }
        let controller = Arc::new(WindowsProcessController::new());
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--child")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let original = match controller.get_priority(child.id()) {
            Ok(p) => p,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            },
        };
        // Normal has an exact reversible mapping; never change an inherited
        // realtime/high class whose public enum might lose OS class precision.
        if original != PolicyPriority::Normal {
            let _ = child.kill();
            let _ = child.wait();
            return Err("child original priority was not Normal".into());
        }
        let mut owned = OwnedChild {
            child,
            controller: controller.clone(),
            original,
        };
        let pid = owned.child.id();
        let mut input = owned.child.stdin.take().ok_or("child stdin missing")?;
        let output = BufReader::new(owned.child.stdout.take().ok_or("child stdout missing")?);
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in output.lines() {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut measure = || {
            observe_workload(WorkloadType::Development, || {
                let mut work = || -> std::io::Result<String> {
                    writeln!(input, "run")?;
                    input.flush()?;
                    receiver
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::TimedOut, e))?
                };
                work().map_err(|e| {
                    aicore_contracts::errors::ContractError::AdapterUnavailable(e.to_string())
                })
            })
        };
        let _warmup = measure()?;
        let telemetry = WindowsTelemetryProvider::new();
        let memory_before = telemetry
            .collect_process(pid)?
            .memory_mb
            .saturating_add(telemetry.collect_process(std::process::id())?.memory_mb);
        let (first, baseline) = measure()?;
        let engine = FeedbackPolicyEngine::new(
            Arc::new(Engine(PolicyPriority::High)),
            Arc::new(Engine(PolicyPriority::Normal)),
        );
        let context = ContextSnapshot {
            workload_type: WorkloadType::Development,
            ..Default::default()
        };
        let snapshot = ComputerSnapshot::default();
        let policy = engine.generate(&snapshot, &context);
        let result = DefaultPolicyExecutor::new(Some(controller.clone()), None).execute(
            &policy,
            &ExecutionTargets {
                process_only: true,
                foreground_pids: vec![pid],
                background_pids: vec![],
            },
        );
        if !result
            .outcomes
            .iter()
            .any(|o| o.resource == "process.priority" && o.status == ExecutionStatus::Success)
            || controller.get_priority(pid)? != PolicyPriority::High
        {
            return Err("native apply/readback failed".into());
        }
        let (second, intervention) = measure()?;
        let memory_after = telemetry
            .collect_process(pid)?
            .memory_mb
            .saturating_add(telemetry.collect_process(std::process::id())?.memory_mb);
        if first.trim().is_empty() || first != second {
            return Err("workload completion mismatch".into());
        }
        controller.set_priority(pid, original)?;
        let restored = controller.get_priority(pid)? == original;
        if !restored {
            return Err("native restoration readback failed".into());
        }
        let now = intervention.timestamp_unix_ms;
        let comparison = EffectComparison {
            policy_id: policy.policy_id.clone(),
            baseline,
            intervention,
            control: None,
            control_overhead_ms: result
                .finished_at_unix_ms
                .saturating_sub(result.started_at_unix_ms),
            slow_loop_overhead_ms: 0,
            memory_overhead_mb: memory_after.saturating_sub(memory_before),
        };
        let assessment = engine.record_effect(
            &policy,
            &comparison,
            OverheadBudget {
                max_control_ms: 100,
                max_slow_ms: 100,
                max_memory_mb: 32,
            },
            now,
        )?;
        let next = engine.generate(&snapshot, &context);
        println!(
            "{}",
            serde_json::json!({"status":"PASS","pid":pid,"changes":["Normal -> High (ABOVE_NORMAL) -> Normal"],"priority_restored":restored,"affinity_power_gpu_npu":"unchanged","observations":comparison,"feedback":assessment,"next_priority":format!("{:?}",next.cpu.priority),"note":"single paired workload; no product performance certification"})
        );
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    return probe::run();
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("NOT VERIFIED: Windows owned-process probe unavailable on this OS");
        Ok(())
    }
}
