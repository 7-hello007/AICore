//! AICore benchmark 工具。
//!
//! 跑 N 次 tick，输出 JSON 结果（延迟分布 + RSS + 线程数）。

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aicore_ai::LocalFeatureProvider;
use aicore_contracts::{
    execution::targets::ExecutionTargets,
    runtime::config::AdaptiveLoopConfig,
    traits::{
        ai_provider::IAiProvider, policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    executor::DefaultPolicyExecutor,
    policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine},
};
use aicore_runtime::AdaptiveLoop;

#[cfg(target_os = "linux")]
use aicore_adapter_linux::{
    LinuxCapabilityProvider, LinuxPowerController, LinuxProcessController, LinuxTelemetryProvider,
};
#[cfg(target_os = "windows")]
use aicore_adapter_windows::{
    WindowsCapabilityProvider, WindowsPowerController, WindowsProcessController,
    WindowsTelemetryProvider,
};

use aicore_contracts::traits::capability_provider::ICapabilityProvider;

#[derive(serde::Serialize)]
struct BenchReport {
    version: String,
    started_at_unix_ms: u64,
    tick_count: usize,
    total_ms: u64,
    avg_tick_ms: f64,
    p50_tick_ms: u64,
    p95_tick_ms: u64,
    p99_tick_ms: u64,
    max_tick_ms: u64,
    min_tick_ms: u64,
    successful_ticks: u64,
    failed_ticks: u64,
    noop_ticks: u64,
    rss_mb_before: u64,
    rss_mb_after: u64,
    rss_growth_mb: i64,
    threads_before: usize,
    threads_after: usize,
    platform: String,
    capabilities_count: usize,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tick_count: usize = args
        .iter()
        .position(|a| a == "--ticks")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    if tick_count == 0 {
        eprintln!("[bench] --ticks must be greater than zero");
        return ExitCode::from(2);
    }

    let output_path: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    println!("[bench] running {} ticks...", tick_count);

    let runtime = build_runtime();
    let targets = ExecutionTargets::default();

    let rss_before = read_rss_mb();
    let threads_before = read_thread_count();

    let mut durations: Vec<u64> = Vec::with_capacity(tick_count);
    let started = Instant::now();

    for _ in 0..tick_count {
        let tick_start = Instant::now();
        let _outcome = runtime.tick(&targets);
        durations.push(tick_start.elapsed().as_millis() as u64);
    }

    let total_ms = started.elapsed().as_millis() as u64;

    let rss_after = read_rss_mb();
    let threads_after = read_thread_count();

    durations.sort_unstable();
    let avg = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
    let p50 = percentile(&durations, 50);
    let p95 = percentile(&durations, 95);
    let p99 = percentile(&durations, 99);
    let max = durations.last().copied().unwrap_or(0);
    let min = durations.first().copied().unwrap_or(0);

    let metrics = runtime.metrics();

    let report = BenchReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        tick_count,
        total_ms,
        avg_tick_ms: avg,
        p50_tick_ms: p50,
        p95_tick_ms: p95,
        p99_tick_ms: p99,
        max_tick_ms: max,
        min_tick_ms: min,
        successful_ticks: metrics.successful_ticks,
        failed_ticks: metrics.failed_ticks,
        noop_ticks: metrics
            .total_ticks
            .saturating_sub(metrics.successful_ticks)
            .saturating_sub(metrics.failed_ticks),
        rss_mb_before: rss_before,
        rss_mb_after: rss_after,
        rss_growth_mb: rss_after as i64 - rss_before as i64,
        threads_before,
        threads_after,
        platform: platform_name().to_string(),
        capabilities_count: capabilities_count(),
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    println!("{}", json);

    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, &json) {
            eprintln!("[bench] write failed: {}", e);
            return ExitCode::from(1);
        }
        eprintln!("[bench] wrote {}", path.display());
    }

    ExitCode::SUCCESS
}

fn percentile(sorted: &[u64], p: u32) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (p as f64 / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn platform_name() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
}

fn capabilities_count() -> usize {
    #[cfg(target_os = "linux")]
    {
        LinuxCapabilityProvider::new().capabilities_detailed().len()
    }
    #[cfg(target_os = "windows")]
    {
        WindowsCapabilityProvider::new()
            .capabilities_detailed()
            .len()
    }
}

fn build_runtime() -> AdaptiveLoop {
    let (telemetry, executor) = build_platform();

    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());

    let ai: Option<Arc<dyn IAiProvider>> = match LocalFeatureProvider::with_default_weights() {
        Ok(p) => Some(Arc::new(p)),
        Err(_) => None,
    };
    let primary: Arc<dyn IPolicyEngine> =
        Arc::new(AiAssistedPolicyEngine::new(base.clone(), ai, 0.6));
    let fallback = base;

    let validator = Arc::new(DefaultPolicyValidator::new());

    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        primary,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
}

#[cfg(target_os = "linux")]
fn build_platform() -> (Arc<dyn ITelemetryProvider>, Arc<dyn IPolicyExecutor>) {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(LinuxTelemetryProvider::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(Arc::new(LinuxProcessController::new())),
        Some(Arc::new(LinuxPowerController::new())),
    ));
    (telemetry, executor)
}

#[cfg(target_os = "windows")]
fn build_platform() -> (Arc<dyn ITelemetryProvider>, Arc<dyn IPolicyExecutor>) {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(WindowsTelemetryProvider::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(Arc::new(WindowsProcessController::new())),
        Some(Arc::new(WindowsPowerController::new())),
    ));
    (telemetry, executor)
}

/// 读取进程 RSS（MB）。
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

#[cfg(target_os = "windows")]
fn read_rss_mb() -> u64 {
    // Windows: 简化，不引入 windows-sys 到 bench
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

#[cfg(target_os = "windows")]
fn read_thread_count() -> usize {
    0
}
