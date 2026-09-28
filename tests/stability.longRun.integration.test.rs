//! Long and steady.
//!
//! Length controlled by `AICORE_LONG_RUN_SECONDS`:
//! - Default 30 seconds (local / CI smoke)
//! - CI nightly：3600
//! - Product acceptance: 86400
//!
//! (no absolute threshold, no variable only)
//! -Tick Count = Expectation
//! - RSS growth ≤ 32MB (reasonable ceiling, container environment)
//! - Thread growth ~ 8 (tokio runtime stable)
//! -p99 pick delay < 500ms
//! - No Panic

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aicore_contracts::{
    errors::ContractResult,
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::resource_policy::ResourcePolicy,
    runtime::config::AdaptiveLoopConfig,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
};
use aicore_runtime::AdaptiveLoop;

struct MockTelemetry {
    calls: AtomicU64,
}

impl MockTelemetry {
    fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
        }
    }
}

impl ITelemetryProvider for MockTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut s = ComputerSnapshot::default();
        s.cpu.logical_cores = 8;
        s.cpu.usage_percent = 25.0;
        s.memory.total_mb = 16_384;
        s.memory.used_mb = 8_192;
        s.memory.usage_percent = 50.0;
        Ok(s)
    }
    fn platform_name(&self) -> &'static str {
        "mock"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
}

struct OkExecutor;

impl IPolicyExecutor for OkExecutor {
    fn execute(&self, p: &ResourcePolicy, _t: &ExecutionTargets) -> ExecutionResult {
        ExecutionResult {
            policy_id: p.policy_id.clone(),
            overall_status: ExecutionOverallStatus::AllSucceeded,
            started_at_unix_ms: 0,
            finished_at_unix_ms: 0,
            outcomes: vec![ExecutionOutcome {
                resource: "test".to_string(),
                target: "system".to_string(),
                status: ExecutionStatus::Success,
                message: "ok".to_string(),
            }],
        }
    }
    fn name(&self) -> &'static str {
        "ok"
    }
}

fn build_loop() -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(MockTelemetry::new());
    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let fallback = base.clone();
    let validator = Arc::new(DefaultPolicyValidator::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(OkExecutor);

    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        base,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
}

fn long_run_seconds() -> u64 {
    std::env::var("AICORE_LONG_RUN_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
}

#[cfg(target_os = "linux")]
fn read_rss_mb() -> u64 {
    let Ok(content) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            return kb / 1024;
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn read_rss_mb() -> u64 {
    0
}

#[cfg(target_os = "linux")]
fn read_thread_count() -> usize {
    let Ok(content) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Threads:") {
            return rest.trim().parse().unwrap_or(0);
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn read_thread_count() -> usize {
    0
}

#[test]
fn long_run_keeps_rss_and_threads_stable() {
    let seconds = long_run_seconds();
    let loop_ = build_loop();
    let targets = ExecutionTargets::default();

    let rss_before = read_rss_mb();
    let threads_before = read_thread_count();

    // Collection delay
    let mut durations: Vec<u64> = Vec::new();
    let start = Instant::now();
    let mut ticks = 0u64;

    while start.elapsed() < Duration::from_secs(seconds) {
        let t0 = Instant::now();
        let _ = loop_.tick(&targets);
        durations.push(t0.elapsed().as_millis() as u64);
        ticks += 1;
        std::thread::sleep(Duration::from_millis(50));
    }

    let rss_after = read_rss_mb();
    let threads_after = read_thread_count();

    durations.sort_unstable();
    let p50 = percentile(&durations, 50);
    let p95 = percentile(&durations, 95);
    let p99 = percentile(&durations, 99);
    let max = durations.last().copied().unwrap_or(0);

    eprintln!(
        "[long-run] seconds={} ticks={} rss_before={}MB rss_after={}MB threads_before={} threads_after={} p50={}ms p95={}ms p99={}ms max={}ms",
        seconds, ticks, rss_before, rss_after, threads_before, threads_after, p50, p95, p99, max
    );

    assert!(ticks > 0, "no ticks executed");

    let metrics = loop_.metrics();
    assert_eq!(metrics.total_ticks, ticks);

    // RSS growth cap: 32MB (container friendly)
    if rss_before > 0 && rss_after > 0 {
        let growth = rss_after as i64 - rss_before as i64;
        assert!(
            growth <= 32,
            "RSS grew by {} MB (before={}, after={})",
            growth,
            rss_before,
            rss_after
        );
    }

    // Maximum linear growth: 8
    if threads_before > 0 && threads_after > 0 {
        let growth = threads_after.saturating_sub(threads_before);
        assert!(
            growth <= 8,
            "thread count grew by {} (before={}, after={})",
            growth,
            threads_before,
            threads_after
        );
    }

    // p99 Delay < 500ms
    assert!(p99 < 500, "p99 latency too high: {} ms", p99);
}

fn percentile(sorted: &[u64], p: u32) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (p as f64 / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
