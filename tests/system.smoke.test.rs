//! Smoke test：验证运行时闭环能连续跑 N 秒不崩溃。
//!
//! 时长由 `AICORE_SMOKE_SECONDS` 控制，默认 5 秒。
//! - 本地开发：默认 5s
//! - CI：60s
//! - 产品验收：可设为 86400（24h）
//!
//! 断言：
//! - 至少执行 1 个 tick
//! - metrics 计数一致
//! - 没有 panic

use std::sync::Arc;
use std::time::{Duration, Instant};

use aicore_contracts::{
    errors::ContractResult,
    execution::targets::ExecutionTargets,
    runtime::config::AdaptiveLoopConfig,
    telemetry::{cpu::CpuSnapshot, memory::MemorySnapshot, snapshot::ComputerSnapshot},
    traits::{
        policy_executor::IPolicyExecutor, runtime_orchestrator::IRuntimeOrchestrator,
        telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    executor::DefaultPolicyExecutor,
    policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
};
use aicore_runtime::AdaptiveLoop;

/// The smoke test checks loop accounting, not host controls or sensors.
struct IsolatedTelemetry;

impl ITelemetryProvider for IsolatedTelemetry {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        Ok(ComputerSnapshot {
            collected_at_unix_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            ),
            cpu: CpuSnapshot {
                logical_cores: 4,
                ..Default::default()
            },
            memory: MemorySnapshot {
                total_mb: 1024,
                available_mb: 1024,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn platform_name(&self) -> &'static str {
        "isolated_smoke"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["cpu.telemetry", "memory.telemetry"]
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn build_loop() -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(IsolatedTelemetry);

    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));
    let base: Arc<dyn aicore_contracts::traits::policy_engine::IPolicyEngine> =
        Arc::new(RuleBasedPolicyEngine::new());
    let primary = base.clone();
    let fallback = base;

    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(None, None));

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

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn smoke_seconds() -> u64 {
    std::env::var("AICORE_SMOKE_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn smoke_runs_without_panic() {
    let seconds = smoke_seconds();
    let loop_ = build_loop();
    let targets = ExecutionTargets::default();

    let start = Instant::now();
    let mut ticks = 0u64;
    while start.elapsed() < Duration::from_secs(seconds) {
        let _outcome = loop_.tick(&targets);
        ticks += 1;
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(ticks > 0, "no ticks executed in {}s", seconds);

    let m = loop_.metrics();
    assert_eq!(m.total_ticks, ticks);
    assert_eq!(
        m.total_ticks,
        m.successful_ticks + m.failed_ticks + m.noop_ticks
    );
    assert_eq!(
        m.noop_ticks, ticks,
        "no controller or target can apply a policy"
    );
    assert_eq!(m.policies_generated, ticks);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn smoke_trace_ids_unique() {
    let loop_ = build_loop();
    let targets = ExecutionTargets::default();

    let mut trace_ids = std::collections::HashSet::new();
    for _ in 0..5 {
        let outcome = loop_.tick(&targets);
        trace_ids.insert(outcome.trace_id);
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(trace_ids.len(), 5, "trace_ids must be unique across ticks");
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn smoke_mode_is_valid_enum_value() {
    // Smoke test 只验证"循环能跑、mode 是合法值"，不断言特定环境结果。
    // 真实环境下 Power/Process 控制可能因权限不足失败，导致进入 Degraded/Safe。
    let loop_ = build_loop();
    let targets = ExecutionTargets::default();

    for _ in 0..3 {
        let _ = loop_.tick(&targets);
        std::thread::sleep(Duration::from_millis(20));
    }

    let mode = loop_.current_mode();
    let valid = matches!(
        mode,
        aicore_contracts::runtime::mode::RuntimeMode::Normal
            | aicore_contracts::runtime::mode::RuntimeMode::Degraded
            | aicore_contracts::runtime::mode::RuntimeMode::Safe
            | aicore_contracts::runtime::mode::RuntimeMode::Recovery
    );
    assert!(valid, "unexpected runtime mode: {:?}", mode);

    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
}
