use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use aicore_contracts::{
    execution::{result::ExecutionResult, targets::ExecutionTargets},
    policy::resource_policy::ResourcePolicy,
    traits::{policy_executor::IPolicyExecutor, policy_validator::IPolicyValidator},
};

/// An immutable proposal from the slow side. The expiry is checked again at
/// the point of execution, so a late inference can never actuate the host.
#[derive(Clone)]
pub struct PreparedPolicy {
    pub policy: Arc<ResourcePolicy>,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone)]
struct PreparedEnvelope {
    generation: u64,
    proposal: PreparedPolicy,
}

type PreparationJob = Box<dyn FnOnce() -> Option<PreparedPolicy> + Send>;
type PreparationRequest = (u64, PreparationJob);

/// One in-flight slow job and one queued job. Cancellation is generation
/// based: a completed job from an earlier generation is discarded.
pub struct SlowPolicyWorker {
    requests: mpsc::SyncSender<PreparationRequest>,
    results: Arc<Mutex<Option<PreparedEnvelope>>>,
    generation: Arc<AtomicU64>,
}

impl SlowPolicyWorker {
    pub fn new() -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<PreparationRequest>(1);
        let results = Arc::new(Mutex::new(None));
        let worker_results = results.clone();
        std::thread::Builder::new()
            .name("aicore-slow-policy".into())
            .spawn(move || {
                while let Ok((generation, job)) = receiver.recv() {
                    let proposal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                        .ok()
                        .flatten();
                    if let Some(proposal) = proposal {
                        if let Ok(mut latest) = worker_results.lock() {
                            *latest = Some(PreparedEnvelope {
                                generation,
                                proposal,
                            });
                        }
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Returns false immediately when the bounded worker is busy.
    pub fn submit(&self, job: impl FnOnce() -> Option<PreparedPolicy> + Send + 'static) -> bool {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.requests.try_send((generation, Box::new(job))).is_ok()
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn latest(&self) -> Option<PreparedEnvelope> {
        self.results
            .lock()
            .ok()?
            .take()
            .filter(|result| result.generation == self.generation.load(Ordering::SeqCst))
    }
}

pub enum FastPolicyOutcome {
    Executed(ExecutionResult),
    Rejected,
}

/// Consumes only precomputed policies. It does no telemetry, history, AI or
/// runtime binding and never waits for the slow worker. The fallback is a
/// caller-supplied rule policy, not a fabricated successful execution.
pub struct FastPolicyConsumer {
    worker: Arc<SlowPolicyWorker>,
    current: Mutex<Option<PreparedEnvelope>>,
    fallback: Arc<ResourcePolicy>,
    validator: Arc<dyn IPolicyValidator>,
    executor: Arc<dyn IPolicyExecutor>,
}

impl FastPolicyConsumer {
    pub fn new(
        worker: Arc<SlowPolicyWorker>,
        fallback: Arc<ResourcePolicy>,
        validator: Arc<dyn IPolicyValidator>,
        executor: Arc<dyn IPolicyExecutor>,
    ) -> Self {
        Self {
            worker,
            current: Mutex::new(None),
            fallback,
            validator,
            executor,
        }
    }

    pub fn tick(&self, now_unix_ms: u64, targets: &ExecutionTargets) -> FastPolicyOutcome {
        if let Some(proposal) = self.worker.latest() {
            if let Ok(mut current) = self.current.lock() {
                *current = Some(proposal);
            }
        }
        let policy = self
            .current
            .lock()
            .ok()
            .and_then(|current| current.clone())
            .filter(|envelope| {
                envelope.generation == self.worker.generation.load(Ordering::SeqCst)
                    && envelope.proposal.expires_at_unix_ms > now_unix_ms
            })
            .map(|envelope| envelope.proposal.policy)
            .unwrap_or_else(|| self.fallback.clone());
        if !self.validator.validate(&policy).accepted {
            return FastPolicyOutcome::Rejected;
        }
        FastPolicyOutcome::Executed(self.executor.execute(&policy, targets))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    use aicore_contracts::{
        execution::status::ExecutionOverallStatus, traits::policy_validator::ValidationResult,
    };

    struct Accept;
    impl IPolicyValidator for Accept {
        fn validate(&self, _: &ResourcePolicy) -> ValidationResult {
            ValidationResult::accept()
        }
        fn name(&self) -> &'static str {
            "accept_mock"
        }
    }
    struct ModelExecutor;
    impl IPolicyExecutor for ModelExecutor {
        fn execute(&self, policy: &ResourcePolicy, _: &ExecutionTargets) -> ExecutionResult {
            ExecutionResult {
                policy_id: policy.policy_id.clone(),
                overall_status: ExecutionOverallStatus::NoOp,
                started_at_unix_ms: 0,
                finished_at_unix_ms: 0,
                outcomes: vec![],
            }
        }
        fn name(&self) -> &'static str {
            "model_mock"
        }
    }

    fn consumer(worker: Arc<SlowPolicyWorker>) -> FastPolicyConsumer {
        FastPolicyConsumer::new(
            worker,
            Arc::new(ResourcePolicy {
                policy_id: "fallback".into(),
                ..Default::default()
            }),
            Arc::new(Accept),
            Arc::new(ModelExecutor),
        )
    }

    fn selected_id(outcome: FastPolicyOutcome) -> String {
        match outcome {
            FastPolicyOutcome::Executed(result) => {
                assert_eq!(result.overall_status, ExecutionOverallStatus::NoOp);
                result.policy_id
            },
            FastPolicyOutcome::Rejected => panic!("mock validator rejected"),
        }
    }

    fn await_prepared(fast: &FastPolicyConsumer) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if selected_id(fast.tick(10, &ExecutionTargets::default())) == "prepared" {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("slow worker never delivered prepared policy");
    }

    #[test]
    fn hung_slow_job_does_not_break_fast_deadline_or_execute_late_result() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        let (started_tx, started_rx) = mpsc::channel();
        assert!(worker.submit(move || {
            let _ = started_tx.send(());
            std::thread::sleep(Duration::from_millis(300));
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "late".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 100,
            })
        }));
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let started = Instant::now();
        assert_eq!(
            selected_id(fast.tick(10, &ExecutionTargets::default())),
            "fallback"
        );
        assert!(started.elapsed() < Duration::from_millis(150));
        worker.cancel();
        std::thread::sleep(Duration::from_millis(320));
        assert_eq!(
            selected_id(fast.tick(10, &ExecutionTargets::default())),
            "fallback"
        );
    }

    #[test]
    fn expired_proposal_falls_back_without_counting_model_as_success() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        assert!(worker.submit(|| {
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "prepared".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 20,
            })
        }));
        await_prepared(&fast);
        assert_eq!(
            selected_id(fast.tick(21, &ExecutionTargets::default())),
            "fallback"
        );
    }

    #[test]
    fn cancellation_revokes_a_previously_delivered_policy() {
        let worker = Arc::new(SlowPolicyWorker::new().unwrap());
        let fast = consumer(worker.clone());
        assert!(worker.submit(|| {
            Some(PreparedPolicy {
                policy: Arc::new(ResourcePolicy {
                    policy_id: "prepared".into(),
                    ..Default::default()
                }),
                expires_at_unix_ms: 100,
            })
        }));
        await_prepared(&fast);
        worker.cancel();
        assert_eq!(
            selected_id(fast.tick(20, &ExecutionTargets::default())),
            "fallback"
        );
    }
}
