use std::path::{Path, PathBuf};
use std::sync::Arc;

use aicore_ai::workload::MlWorkloadSource;
use aicore_ai_runtime::{PureRustRuntime, RuntimeBackedProvider};

use aicore_contracts::{
    capability::entry::CapabilityEntry,
    execution::targets::ExecutionTargets,
    memory::retention::RetentionPolicy,
    runtime::config::AdaptiveLoopConfig,
    traits::{
        ai_runtime::IAiRuntime, policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        runtime_binder::IRuntimeBinder, task_queue::ITaskQueue,
        telemetry_provider::ITelemetryProvider,
    },
};

use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    event_bus::EventBus,
    executor::{DefaultPolicyExecutor, TransparentExecutor},
    model::{
        ModelEnvironment, ModelPolicyExecutor, ModelPostConditionChecker, ModelTargetProvider,
    },
    policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine},
    prediction::ShortHorizonPredictor,
};

use aicore_memory::{RetentionScheduler, SqliteMemoryStore};

use aicore_runtime::{AdaptiveLoop, ModelRuntimeBinder};

use aicore_runtime_router::{DefaultRuntimeBinder, SafeRuntimeRouter};

use aicore_tier::TierHub;
use aicore_tier_l0::register_l0_with_fusion;
use aicore_tier_l1::register_l1;
use aicore_tier_l2::register_l2;

/// 组装好的 daemon 运行时。
pub struct DaemonRuntime {
    pub loop_: AdaptiveLoop,
    pub platform_name: &'static str,
    pub capabilities: Vec<CapabilityEntry>,
    pub retention: Option<Arc<RetentionScheduler>>,

    /// 当前执行模式使用的 target。
    ///
    /// Model mode：
    /// 使用虚拟 PID，只存在于模型环境中。
    ///
    /// Platform mode：
    /// 使用平台执行目标。
    pub execution_targets: ExecutionTargets,
}

/// 完整构造：memory + task_queue。
pub fn build_runtime_full(
    memory_path: Option<&Path>,
    task_queue: Option<Arc<dyn ITaskQueue>>,
) -> DaemonRuntime {
    let model_mode = model_execution_mode();

    /*
     * ============================================================
     * 1. 平台基础组件
     * ============================================================
     */

    let (telemetry, platform_executor, platform_name) = build_platform_components();

    /*
     * ============================================================
     * 2. Policy Executor
     *
     * Model mode：
     *     ModelPolicyExecutor
     *
     * Platform mode：
     *     DefaultPolicyExecutor
     *
     * 重要：
     * Model mode 永远不会调用真实 ProcessController /
     * PowerController。
     * ============================================================
     */

    let (executor, model_environment): (Arc<dyn IPolicyExecutor>, Option<Arc<ModelEnvironment>>) =
        if model_mode {
            let environment = ModelEnvironment::new();

            let model_executor = ModelPolicyExecutor::new(environment.clone());

            tracing::info!(
                component = "aicored",
                event = "execution_mode",
                mode = "model",
                executor = "model_policy_executor",
                "using simulated execution environment"
            );

            (
                Arc::new(model_executor) as Arc<dyn IPolicyExecutor>,
                Some(environment),
            )
        } else {
            tracing::info!(
                component = "aicored",
                event = "execution_mode",
                mode = "platform",
                executor = "default_policy_executor",
                "using platform execution environment"
            );

            (platform_executor, None)
        };

    /*
     * ============================================================
     * 3. Context / Tier
     * ============================================================
     */

    let ctx_engine =
        Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new())).with_max_active_apps(16));

    let tier_hub = TierHub::new();

    let ml_source = MlWorkloadSource::with_default_weights().map(|s| {
        Arc::new(s) as Arc<dyn aicore_contracts::traits::workload_source::IWorkloadSource>
    });

    let _ = register_l0_with_fusion(&tier_hub, ml_source.into_iter().collect());

    let _ = register_l1(&tier_hub);
    let _ = register_l2(&tier_hub);

    let tier_hub = Arc::new(tier_hub);

    /*
     * ============================================================
     * 4. AI Runtime
     *
     * Production default：
     * PureRustRuntime + local JSON model。
     *
     * 不使用 Mock AI 作为生产默认。
     * ============================================================
     */

    let shared_ai_runtime: Option<Arc<PureRustRuntime>> = {
        let model_path = model_path();

        let runtime = Arc::new(PureRustRuntime::new());

        let spec = aicore_contracts::ai::runtime::ModelSpec {
            name: "policy_classifier_v1".to_string(),
            version: "1.0.0".to_string(),
            kind: aicore_contracts::ai::runtime::ModelKind::FeatureWeighted,
            source: aicore_contracts::ai::runtime::ModelSource::File {
                path: model_path.display().to_string(),
            },
            onnx_spec: None,
        };

        match runtime.load_model(&spec) {
            Ok(_) => Some(runtime),

            Err(e) => {
                tracing::warn!(
                    component = "aicored",
                    event = "real_model_init_failed",
                    error = %e,
                    "falling back to rule-based policy engine only"
                );

                None
            },
        }
    };

    let ai: Option<Arc<dyn aicore_contracts::traits::ai_provider::IAiProvider>> =
        shared_ai_runtime.as_ref().and_then(|runtime| {
            let spec = aicore_contracts::ai::runtime::ModelSpec {
                name: "policy_classifier_v1".to_string(),
                version: "1.0.0".to_string(),
                kind: aicore_contracts::ai::runtime::ModelKind::FeatureWeighted,
                source: aicore_contracts::ai::runtime::ModelSource::File {
                    path: model_path().display().to_string(),
                },
                onnx_spec: None,
            };

            runtime
                .loaded_models()
                .into_iter()
                .find(|h| h.model_name == spec.name)
                .map(|handle| {
                    Arc::new(RuntimeBackedProvider::new(runtime.clone(), handle, 4))
                        as Arc<dyn aicore_contracts::traits::ai_provider::IAiProvider>
                })
        });

    /*
     * ============================================================
     * 5. Policy Engine
     * ============================================================
     */

    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());

    let primary: Arc<dyn IPolicyEngine> =
        Arc::new(AiAssistedPolicyEngine::new(base.clone(), ai.clone(), 0.6));

    let fallback = base;

    let validator = Arc::new(DefaultPolicyValidator::new());

    /*
     * ============================================================
     * 6. Event Bus + Transparent Executor
     * ============================================================
     */

    let event_bus = Arc::new(EventBus::new());

    let transparent: Arc<TransparentExecutor> = if let Some(environment) = model_environment.clone()
    {
        Arc::new(
            TransparentExecutor::new(executor.clone())
                .with_checker(Arc::new(ModelPostConditionChecker::new(environment))),
        )
    } else {
        Arc::new(TransparentExecutor::new(executor.clone()))
    };

    /*
     * ============================================================
     * 7. AdaptiveLoop
     * ============================================================
     */

    let loop_ = AdaptiveLoop::new(
        telemetry,
        ctx_engine,
        primary,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_event_bus(event_bus)
    .with_transparent_executor(transparent);

    /*
     * ============================================================
     * 8. Memory
     * ============================================================
     */

    let mut memory_for_retention: Option<
        Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore>,
    > = None;

    let loop_ = if let Some(path) = memory_path {
        match SqliteMemoryStore::open(path) {
            Ok(store) => {
                tracing::info!(
                    component = "aicored",
                    event = "memory_store_opened",
                    path = %path.display(),
                    "sqlite memory store opened"
                );

                let memory: Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore> =
                    Arc::new(store);

                memory_for_retention = Some(memory.clone());

                loop_.with_memory(memory)
            },

            Err(e) => {
                tracing::warn!(
                    component = "aicored",
                    event = "memory_store_open_failed",
                    path = %path.display(),
                    error = %e,
                    "running without memory"
                );

                loop_
            },
        }
    } else {
        loop_
    };

    /*
     * ============================================================
     * 9. Predictor
     * ============================================================
     */

    let predictor: Arc<dyn aicore_contracts::traits::predictor::IPredictor> =
        Arc::new(ShortHorizonPredictor::with_defaults());

    let loop_ = loop_.with_predictor(predictor);

    /*
     * ============================================================
     * 10. Runtime Routing
     * ============================================================
     */

    let router: Arc<dyn aicore_contracts::traits::runtime_router::IRuntimeRouter> =
        Arc::new(SafeRuntimeRouter::new());

    let loop_ = loop_
        .with_runtime_router(router)
        .with_tier_hub(tier_hub.clone());

    /*
     * ============================================================
     * 11. Runtime Binder
     *
     * Model mode：
     *     ModelRuntimeBinder
     *
     * Platform mode：
     *     DefaultRuntimeBinder
     *
     * 这样 Model mode 下 GPU / NPU 不会碰真实硬件。
     * ============================================================
     */

    let binder: Arc<dyn IRuntimeBinder> = if model_mode {
        let mut model_binder = ModelRuntimeBinder::new();

        if let Some(runtime) = shared_ai_runtime.clone() {
            if let Some(handle) = runtime
                .loaded_models()
                .into_iter()
                .find(|h| h.model_name == "policy_classifier_v1")
            {
                model_binder = model_binder.with_ai_runtime(runtime, handle);
            }
        }

        tracing::info!(
            component = "aicored",
            event = "runtime_binder_selected",
            binder = "model_runtime_binder",
            "using simulated runtime binding"
        );

        Arc::new(model_binder)
    } else {
        let mut platform_binder = DefaultRuntimeBinder::new();

        if let Some(runtime) = shared_ai_runtime.clone() {
            if let Some(handle) = runtime
                .loaded_models()
                .into_iter()
                .find(|h| h.model_name == "policy_classifier_v1")
            {
                platform_binder = platform_binder.with_ai_runtime(runtime, handle);
            }
        }

        /*
         * Linux real hardware binding.
         *
         * 只有 platform mode 才允许进入这里。
         */
        #[cfg(target_os = "linux")]
        {
            use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
            let exec = Arc::new(aicore_adapter_linux::LinuxHardwareExecutor::new())
                as Arc<dyn aicore_contracts::traits::hardware_executor::IHardwareExecutor>;
            let hardware_telemetry = aicore_adapter_linux::LinuxHardwareTelemetryProvider::new();
            let capability = aicore_adapter_linux::LinuxHardwareCapabilityProvider::new();
            for device in hardware_telemetry.gpu_devices() {
                platform_binder =
                    platform_binder.with_hardware(capability.gpu_capability(&device.id));
            }
            for device in hardware_telemetry.npu_devices() {
                platform_binder =
                    platform_binder.with_hardware(capability.npu_capability(&device.id));
            }
            platform_binder = platform_binder
                .with_gpu_executor(exec.clone())
                .with_npu_executor(exec);
        }

        /*
         * Windows real hardware binding。
         *
         * 只有 platform mode 才允许进入这里。
         */
        #[cfg(target_os = "windows")]
        {
            use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
            let exec = Arc::new(aicore_adapter_windows::WindowsHardwareExecutor::new())
                as Arc<dyn aicore_contracts::traits::hardware_executor::IHardwareExecutor>;
            let hardware_telemetry =
                aicore_adapter_windows::WindowsHardwareTelemetryProvider::new();
            let capability = aicore_adapter_windows::WindowsHardwareCapabilityProvider::new();
            for device in hardware_telemetry.gpu_devices() {
                platform_binder =
                    platform_binder.with_hardware(capability.gpu_capability(&device.id));
            }
            for device in hardware_telemetry.npu_devices() {
                platform_binder =
                    platform_binder.with_hardware(capability.npu_capability(&device.id));
            }
            platform_binder = platform_binder
                .with_gpu_executor(exec.clone())
                .with_npu_executor(exec);
        }

        tracing::info!(
            component = "aicored",
            event = "runtime_binder_selected",
            binder = "default_runtime_binder",
            "using platform runtime binding"
        );

        Arc::new(platform_binder)
    };

    let loop_ = loop_.with_runtime_binder(binder);

    /*
     * ============================================================
     * 12. Task Queue
     * ============================================================
     */

    let loop_ = if let Some(q) = task_queue {
        loop_.with_task_queue(q)
    } else {
        loop_
    };

    /*
     * ============================================================
     * 13. Capabilities
     * ============================================================
     */

    let capabilities = build_capabilities();

    /*
     * ============================================================
     * 14. Retention
     * ============================================================
     */

    let retention: Option<Arc<RetentionScheduler>> = memory_for_retention.and_then(|store| {
        let policy = RetentionPolicy::default();

        match RetentionScheduler::new(store, policy) {
            Ok(s) => Some(Arc::new(s)),

            Err(e) => {
                tracing::warn!(
                    component = "aicored",
                    event = "retention_init_failed",
                    error = %e,
                    "retention scheduler not started"
                );

                None
            },
        }
    });

    /*
     * ============================================================
     * 15. Execution Targets
     *
     * 这一步必须在 DaemonRuntime 返回之前完成。
     *
     * Model mode：
     *     虚拟 foreground/background PID。
     *
     * Platform mode：
     *     保持原有空 target 行为，未来由上层提供真实 target。
     * ============================================================
     */

    let execution_targets = if model_mode {
        ModelTargetProvider::default().execution_targets()
    } else {
        ExecutionTargets::default()
    };

    /*
     * ============================================================
     * 16. Final Runtime
     * ============================================================
     */

    DaemonRuntime {
        loop_,
        platform_name,
        capabilities,
        retention,
        execution_targets,
    }
}

/// 模型文件路径。
fn model_path() -> PathBuf {
    if let Ok(path) = std::env::var("AICORE_MODEL_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("models").join("policy_classifier_v1.json");

            if candidate.exists() {
                return candidate;
            }

            let candidate = parent
                .join("..")
                .join("..")
                .join("models")
                .join("policy_classifier_v1.json");

            if candidate.exists() {
                return candidate;
            }
        }
    }

    Path::new("models/policy_classifier_v1.json").to_path_buf()
}

#[cfg(target_os = "linux")]
fn build_platform_components() -> (
    Arc<dyn ITelemetryProvider>,
    Arc<dyn IPolicyExecutor>,
    &'static str,
) {
    use aicore_adapter_linux::{
        LinuxPowerController, LinuxProcessController, LinuxTelemetryProvider,
    };

    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(LinuxTelemetryProvider::new());

    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(Arc::new(LinuxProcessController::new())),
        Some(Arc::new(LinuxPowerController::new())),
    ));

    (telemetry, executor, "linux")
}

#[cfg(target_os = "windows")]
fn build_platform_components() -> (
    Arc<dyn ITelemetryProvider>,
    Arc<dyn IPolicyExecutor>,
    &'static str,
) {
    use aicore_adapter_windows::{
        WindowsPowerController, WindowsProcessController, WindowsTelemetryProvider,
    };

    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(WindowsTelemetryProvider::new());

    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(Arc::new(WindowsProcessController::new())),
        Some(Arc::new(WindowsPowerController::new())),
    ));

    (telemetry, executor, "windows")
}

#[cfg(target_os = "linux")]
fn build_capabilities() -> Vec<CapabilityEntry> {
    use aicore_adapter_linux::LinuxCapabilityProvider;
    use aicore_contracts::traits::capability_provider::ICapabilityProvider;

    LinuxCapabilityProvider::new().capabilities_detailed()
}

#[cfg(target_os = "windows")]
fn build_capabilities() -> Vec<CapabilityEntry> {
    use aicore_adapter_windows::WindowsCapabilityProvider;
    use aicore_contracts::traits::capability_provider::ICapabilityProvider;

    WindowsCapabilityProvider::new().capabilities_detailed()
}

/// AICore 执行模式。
///
/// 默认：model
///
/// Model mode：
/// - 不修改真实进程。
/// - 不修改真实电源状态。
/// - GPU/NPU 只做模型化执行。
/// - Policy 写入 ModelEnvironment。
///
/// Platform mode：
/// - 使用真实平台 adapter。
///
/// 环境变量：
///
/// AICORE_EXECUTION_MODE=model
/// AICORE_EXECUTION_MODE=platform
fn model_execution_mode() -> bool {
    match std::env::var("AICORE_EXECUTION_MODE") {
        Ok(value) if value.eq_ignore_ascii_case("platform") => false,

        Ok(value) if value.eq_ignore_ascii_case("native") => false,

        Ok(value) if value.eq_ignore_ascii_case("model") => true,

        _ => true,
    }
}
