use std::sync::Arc;

use aicore_ai::MockAiProvider;
use aicore_contracts::{
    execution::targets::ExecutionTargets,
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

#[cfg(target_os = "linux")]
use aicore_adapter_linux::{LinuxPowerController, LinuxProcessController};

#[cfg(target_os = "windows")]
use aicore_adapter_windows::{WindowsPowerController, WindowsProcessController};

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn build_full_loop(memory: Arc<InMemoryStore>) -> AdaptiveLoop {
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

    let executor: Arc<dyn IPolicyExecutor> = {
        #[cfg(target_os = "linux")]
        {
            let process = Arc::new(LinuxProcessController::new());
            let power = Arc::new(LinuxPowerController::new());
            Arc::new(DefaultPolicyExecutor::new(Some(process), Some(power)))
        }
        #[cfg(target_os = "windows")]
        {
            let process = Arc::new(WindowsProcessController::new());
            let power = Arc::new(WindowsPowerController::new());
            Arc::new(DefaultPolicyExecutor::new(Some(process), Some(power)))
        }
    };

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
    let loop_ = build_full_loop(memory.clone());

    for _ in 0..3 {
        let _outcome = loop_.tick(&ExecutionTargets::default());
    }

    let m = loop_.metrics();
    assert_eq!(m.total_ticks, 3);
    assert_eq!(m.policies_generated, 3);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn history_disabled_by_default_means_no_records() {
    let memory = Arc::new(InMemoryStore::new());
    let loop_ = build_full_loop(memory.clone());

    loop_.tick(&ExecutionTargets::default());

    // 默认 profile.history_enabled = false，不应写入
    assert_eq!(memory.record_count().unwrap(), 0);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn history_enabled_records_tick() {
    let memory = Arc::new(InMemoryStore::new());
    // 用户显式启用历史记录
    let profile = UserProfile {
        history_enabled: true,
        ..UserProfile::default()
    };
    memory.save_profile(&profile).unwrap();

    let loop_ = build_full_loop(memory.clone());
    loop_.tick(&ExecutionTargets::default());

    assert_eq!(memory.record_count().unwrap(), 1);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn loop_starts_in_normal_mode() {
    let memory = Arc::new(InMemoryStore::new());
    let loop_ = build_full_loop(memory);
    assert_eq!(
        loop_.current_mode(),
        aicore_contracts::runtime::mode::RuntimeMode::Normal
    );
}
