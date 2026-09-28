use std::sync::Arc;

use aicore_ai::MockAiProvider;
use aicore_contracts::{
    memory::profile::UserProfile,
    runtime::config::AdaptiveLoopConfig,
    traits::{
        memory_store::IMemoryStore, policy_executor::IPolicyExecutor,
        runtime_orchestrator::IRuntimeOrchestrator, telemetry_provider::ITelemetryProvider,
    },
};
use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    executor::DefaultPolicyExecutor,
    policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine},
};
use aicore_memory::InMemoryStore;
use aicore_runtime::AdaptiveLoop;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "native.fixture.rs"]
mod native_fixture;

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn build_full_loop(
    memory: Arc<InMemoryStore>,
    process: Arc<native_fixture::OwnedProcess>,
) -> AdaptiveLoop {
    let telemetry: Arc<dyn ITelemetryProvider> = {
        #[cfg(target_os = "linux")]
        {
            Arc::new(aicore_adapter_linux::LinuxTelemetryProvider::new())
        }
        #[cfg(target_os = "windows")]
        {
            Arc::new(aicore_adapter_windows::WindowsTelemetryProvider::new())
        }
    };

    let ctx_engine = Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())));

    let base: Arc<dyn aicore_contracts::traits::policy_engine::IPolicyEngine> =
        Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let primary: Arc<dyn aicore_contracts::traits::policy_engine::IPolicyEngine> =
        Arc::new(AiAssistedPolicyEngine::new(base.clone(), Some(ai), 0.6));
    let fallback = base;

    let executor: Arc<dyn IPolicyExecutor> =
        Arc::new(DefaultPolicyExecutor::new(Some(process), None));

    let validator = Arc::new(DefaultPolicyValidator::new());

    let memory_dyn: Arc<dyn IMemoryStore> = memory;
    AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        primary,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_memory(memory_dyn)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn full_loop_runs_and_records_metrics() {
    let memory = Arc::new(InMemoryStore::new());
    let process = Arc::new(native_fixture::OwnedProcess::new());
    let loop_ = build_full_loop(memory.clone(), process.clone());

    for _ in 0..3 {
        let _outcome = loop_.tick(&process.targets());
    }

    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
    assert_eq!(m.policies_generated, 3);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn history_disabled_by_default_means_no_records() {
    let memory = Arc::new(InMemoryStore::new());
    let process = Arc::new(native_fixture::OwnedProcess::new());
    let loop_ = build_full_loop(memory.clone(), process.clone());

    loop_.tick(&process.targets());

    // Default profile. history_enabled = false, not written
    assert_eq!(memory.record_count().unwrap(), 0);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn history_enabled_records_tick() {
    let memory = Arc::new(InMemoryStore::new());
    // User explicitly enables history
    let profile = UserProfile {
        history_enabled: true,
        ..UserProfile::default()
    };
    memory.save_profile(&profile).unwrap();

    let process = Arc::new(native_fixture::OwnedProcess::new());
    let loop_ = build_full_loop(memory.clone(), process.clone());
    loop_.tick(&process.targets());

    assert_eq!(memory.record_count().unwrap(), 1);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn loop_starts_in_normal_mode() {
    let memory = Arc::new(InMemoryStore::new());
    let process = Arc::new(native_fixture::OwnedProcess::new());
    let loop_ = build_full_loop(memory, process.clone());
    assert_eq!(
        loop_.current_mode(),
        aicore_contracts::runtime::mode::RuntimeMode::Normal
    );
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn native_platform_fast_deadline_measurement() {
    use aicore_contracts::execution::status::ExecutionOverallStatus;
    use aicore_runtime::dual_loop::PlatformTickDriver;
    use std::time::{Duration, Instant};
    let process = Arc::new(native_fixture::OwnedProcess::new());
    let loop_ = Arc::new(build_full_loop(
        Arc::new(InMemoryStore::new()),
        process.clone(),
    ));
    let driver = PlatformTickDriver::new(loop_, process.targets(), Duration::from_secs(3))
        .expect("native split driver");
    let mut samples = Vec::new();
    let mut applied = 0;
    for _ in 0..200 {
        let start = Instant::now();
        let outcome = driver.tick();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        if matches!(
            outcome.execution_status.as_ref(),
            Some(ExecutionOverallStatus::AllSucceeded | ExecutionOverallStatus::PartiallySucceeded)
        ) {
            applied += 1;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    samples.sort_by(f64::total_cmp);
    let misses = samples.iter().filter(|v| **v > 20.0).count();
    eprintln!("NATIVE DEADLINE samples={} applied={applied} budget_ms=20 p50_ms={} p99_ms={} max_ms={} misses={misses}; finite measurement does not certify hard realtime",samples.len(),samples[100],samples[198],samples[199]);
    drop(driver);
    process.finish().expect("native deadline probe restoration");
    assert!(
        applied > 0,
        "native split driver must actually apply a prepared policy"
    );
}
#[cfg(target_os = "windows")]
#[test]
fn native_owned_child_affinity_roundtrip() {
    use aicore_contracts::traits::process_controller::{AffinityMask, IProcessController};
    let process = native_fixture::OwnedProcess::new();
    let pid = process.targets().foreground_pids[0];
    let original = process.get_affinity(pid).expect("native affinity snapshot");
    let single = AffinityMask::new(vec![original.cores[0]]);
    process
        .set_affinity(pid, &single)
        .expect("owned child affinity write");
    assert_eq!(
        process.get_affinity(pid).expect("native affinity readback"),
        single
    );
    process.finish().expect("native affinity restoration");
}
