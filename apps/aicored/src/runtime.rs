use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use aicore_adapter_simulated_platform::SimulatedPlatform;
use aicore_ai::{workload::MlWorkloadSource, BoundedAiProvider};
use aicore_ai_runtime::{
    DeviceRuntime, DeviceRuntimeCatalog, ModelDevice, PureRustRuntime, RuntimeBackedProvider,
};

use aicore_contracts::{
    capability::entry::CapabilityEntry,
    execution::targets::ExecutionTargets,
    hardware::matrix::DeviceCapabilityMatrix,
    memory::retention::RetentionPolicy,
    runtime::config::AdaptiveLoopConfig,
    traits::{
        ai_runtime::IAiRuntime, capability_provider::ICapabilityProvider,
        policy_engine::IPolicyEngine, policy_executor::IPolicyExecutor,
        post_condition_checker::IPostConditionChecker, power_controller::IPowerController,
        process_controller::IProcessController, runtime_binder::IRuntimeBinder,
        task_queue::ITaskQueue, telemetry_provider::ITelemetryProvider,
    },
};

use aicore_core::{
    context::{ContextEngine, RuleBasedClassifier},
    event_bus::EventBus,
    executor::{DefaultPolicyExecutor, PlatformPostConditionChecker, TransparentExecutor},
    model::{
        ModelEnvironment, ModelPolicyExecutor, ModelPostConditionChecker, ModelTargetProvider,
    },
    policy::{
        AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine, ThermalPowerGuard,
    },
    prediction::ShortHorizonPredictor,
};

use aicore_memory::{RetentionScheduler, SqliteMemoryStore};

use aicore_runtime::{AdaptiveLoop, ModelRuntimeBinder};

use aicore_runtime_router::{DefaultRuntimeBinder, SafeRuntimeRouter};

use aicore_tier::TierHub;
use aicore_tier_l0::register_l0_with_fusion;
use aicore_tier_l1::register_l1;
use aicore_tier_l1::L1TaskSource;
use aicore_tier_l2::{register_l2, register_l2_with_queue, L2NativeTaskProvider};

/// 组装好的 daemon 运行时。
pub struct DaemonRuntime {
    pub loop_: Arc<AdaptiveLoop>,
    pub model_mode: bool,
    pub simulated_platform: bool,
    #[cfg(test)]
    pub(crate) simulator: Option<Arc<SimulatedPlatform>>,
    pub platform_name: &'static str,
    pub capabilities: Vec<CapabilityEntry>,
    pub retention: Option<Arc<RetentionScheduler>>,
    pub l1_source: Option<Arc<L1TaskSource>>,
    pub l2_provider: Option<Arc<L2NativeTaskProvider>>,

    /// 当前执行模式使用的 target。
    ///
    /// Model mode：
    /// 使用虚拟 PID，只存在于模型环境中。
    ///
    /// Platform mode：
    /// 使用平台执行目标。
    pub execution_targets: ExecutionTargets,
    pub(crate) telemetry_selection: Arc<TelemetrySelection>,
    pub(crate) process_selection: Arc<ProcessSelection>,
    /// Read-only platform declarations; empty in Model Mode.
    pub hardware_matrices: Vec<DeviceCapabilityMatrix>,
    pub(crate) memory_store: Option<Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore>>,
}

/// Selects a verified typed plugin source when configured; a crashed or
/// invalid plugin cannot prevent the platform's read-only observation path.
pub(crate) struct TelemetrySelection {
    fallback: Arc<dyn ITelemetryProvider>,
    selected: RwLock<Option<Arc<dyn ITelemetryProvider>>>,
}

impl TelemetrySelection {
    fn new(fallback: Arc<dyn ITelemetryProvider>) -> Self {
        Self {
            fallback,
            selected: RwLock::new(None),
        }
    }

    pub fn bind(&self, provider: Arc<dyn ITelemetryProvider>) {
        if let Ok(mut selected) = self.selected.write() {
            *selected = Some(provider);
        }
    }
}

impl ITelemetryProvider for TelemetrySelection {
    fn collect(
        &self,
    ) -> aicore_contracts::errors::ContractResult<
        aicore_contracts::telemetry::snapshot::ComputerSnapshot,
    > {
        let selected = self
            .selected
            .read()
            .ok()
            .and_then(|selection| selection.clone());
        if let Some(provider) = selected {
            match provider.collect() {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => {
                    tracing::warn!(component="aicored", event="plugin_telemetry_fallback", error=%error);
                    if let Ok(mut selection) = self.selected.write() {
                        *selection = None;
                    }
                },
            }
        }
        self.fallback.collect()
    }

    fn platform_name(&self) -> &'static str {
        "selected_telemetry"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        self.fallback.capabilities()
    }
}

/// A selected control plugin never silently falls back to native writes after
/// a crash or bad response. Readback remains independent via the OS adapter.
pub(crate) struct ProcessSelection {
    platform: Arc<dyn IProcessController>,
    selected: RwLock<Option<Arc<dyn IProcessController>>>,
}

impl ProcessSelection {
    fn new(platform: Arc<dyn IProcessController>) -> Self {
        Self {
            platform,
            selected: RwLock::new(None),
        }
    }

    pub fn bind(
        &self,
        controller: Arc<dyn IProcessController>,
    ) -> aicore_contracts::errors::ContractResult<()> {
        let mut selected = self.selected.write().map_err(|_| {
            aicore_contracts::errors::ContractError::AdapterUnavailable(
                "process selection lock poisoned".into(),
            )
        })?;
        *selected = Some(controller);
        Ok(())
    }
}

impl IProcessController for ProcessSelection {
    fn set_priority(
        &self,
        pid: u32,
        priority: aicore_contracts::policy::priority::PolicyPriority,
    ) -> aicore_contracts::errors::ContractResult<()> {
        let selected = self.selected.read().map_err(|_| {
            aicore_contracts::errors::ContractError::AdapterUnavailable(
                "process selection lock poisoned".into(),
            )
        })?;
        match selected.as_ref() {
            Some(plugin) => plugin.set_priority(pid, priority),
            None => self.platform.set_priority(pid, priority),
        }
    }

    fn get_priority(
        &self,
        pid: u32,
    ) -> aicore_contracts::errors::ContractResult<aicore_contracts::policy::priority::PolicyPriority>
    {
        self.platform.get_priority(pid)
    }

    fn set_affinity(
        &self,
        pid: u32,
        mask: &aicore_contracts::traits::process_controller::AffinityMask,
    ) -> aicore_contracts::errors::ContractResult<()> {
        let selected = self.selected.read().map_err(|_| {
            aicore_contracts::errors::ContractError::AdapterUnavailable(
                "process selection lock poisoned".into(),
            )
        })?;
        if selected.is_some() {
            return Err(
                aicore_contracts::errors::ContractError::CapabilityUnavailable(
                    "typed process plugin does not bind affinity".into(),
                ),
            );
        }
        self.platform.set_affinity(pid, mask)
    }

    fn get_affinity(
        &self,
        pid: u32,
    ) -> aicore_contracts::errors::ContractResult<
        aicore_contracts::traits::process_controller::AffinityMask,
    > {
        self.platform.get_affinity(pid)
    }

    fn capabilities(&self) -> Vec<&'static str> {
        let mut capabilities = self.platform.capabilities();
        match self.selected.read() {
            Ok(selected) if selected.is_some() => {
                capabilities.retain(|capability| *capability != "process.affinity.write");
                capabilities
            },
            Ok(_) => capabilities,
            Err(_) => Vec::new(),
        }
    }

    fn platform_name(&self) -> &'static str {
        "selected_process_controller"
    }
}

/// 完整构造：memory + task_queue。
pub fn build_runtime_full(
    memory_path: Option<&Path>,
    task_queue: Option<Arc<dyn ITaskQueue>>,
) -> DaemonRuntime {
    build_runtime_for_mode(memory_path, task_queue, model_execution_mode())
}

pub(crate) fn build_runtime_for_mode(
    memory_path: Option<&Path>,
    task_queue: Option<Arc<dyn ITaskQueue>>,
    model_mode: bool,
) -> DaemonRuntime {
    build_runtime_for_mode_with_simulation(
        memory_path,
        task_queue,
        model_mode,
        simulated_platform_mode(),
    )
}

fn build_runtime_for_mode_with_simulation(
    memory_path: Option<&Path>,
    task_queue: Option<Arc<dyn ITaskQueue>>,
    model_mode: bool,
    simulation: bool,
) -> DaemonRuntime {
    /*
     * ============================================================
     * 1. 平台基础组件
     * ============================================================
     */

    let simulated_platform = if !model_mode && simulation {
        let fixture = bundled_platform_fixture_json();
        match SimulatedPlatform::from_json(fixture) {
            Ok(platform) => Some(Arc::new(platform)),
            Err(error) => panic!("bundled platform simulator fixture invalid: {error}"),
        }
    } else {
        None
    };
    let (telemetry, process_selection, platform_executor, platform_checker, platform_name) =
        if let Some(platform) = &simulated_platform {
            build_simulated_components(platform.clone())
        } else {
            build_platform_components()
        };

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
                mode = if simulated_platform.is_some() {
                    "simulated-platform"
                } else {
                    "platform"
                },
                executor = "default_policy_executor",
                "using selected platform adapter"
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

    let l1_source = match register_l1(&tier_hub) {
        Ok((_, source)) => Some(source),
        Err(error) => {
            tracing::warn!(component = "aicored", event = "l1_register_failed", error = %error);
            None
        },
    };
    let l2_registration = if let Some(queue) = task_queue.clone() {
        register_l2_with_queue(&tier_hub, queue)
    } else {
        register_l2(&tier_hub)
    };
    let l2_provider = match l2_registration {
        Ok((_, provider)) => Some(provider),
        Err(error) => {
            tracing::warn!(component = "aicored", event = "l2_register_failed", error = %error);
            None
        },
    };

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

    let ai_provider: Option<Arc<dyn aicore_contracts::traits::ai_provider::IAiProvider>> =
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

    let ai = ai_provider.and_then(|provider| {
        match BoundedAiProvider::new(provider, Duration::from_millis(10)) {
            Ok(bounded) => Some(Arc::new(bounded)
                as Arc<dyn aicore_contracts::traits::ai_provider::IAiProvider>),
            Err(error) => {
                tracing::warn!(component = "aicored", event = "ai_worker_init_failed", error = %error, "rule-based policy remains available");
                None
            },
        }
    });

    /*
     * ============================================================
     * 5. Policy Engine
     * ============================================================
     */

    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());

    let proposed: Arc<dyn IPolicyEngine> =
        Arc::new(AiAssistedPolicyEngine::new(base.clone(), ai.clone(), 0.6));
    let feedback_engine = Arc::new(aicore_core::policy::FeedbackPolicyEngine::new(
        proposed,
        base.clone(),
    ));
    let primary: Arc<dyn IPolicyEngine> = feedback_engine.clone();

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
        Arc::new(TransparentExecutor::new(executor.clone()).with_checker(platform_checker))
    };

    /*
     * ============================================================
     * 7. AdaptiveLoop
     * ============================================================
     */

    let telemetry_selection = Arc::new(TelemetrySelection::new(telemetry));
    let loop_ = AdaptiveLoop::new(
        telemetry_selection.clone(),
        ctx_engine,
        primary,
        fallback,
        validator,
        executor,
        AdaptiveLoopConfig::default(),
    )
    .with_event_bus(event_bus)
    .with_transparent_executor(transparent)
    .with_thermal_guard(Arc::new(ThermalPowerGuard::default()));
    let loop_ = loop_
        .with_effect_feedback(feedback_engine)
        .with_observed_effects_enabled(!model_mode && !simulation);

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
                let mut catalog = DeviceRuntimeCatalog::new();
                if let Err(error) = catalog.register(DeviceRuntime {
                    device: ModelDevice::Cpu,
                    device_id: "local-cpu".into(),
                    model_kind: aicore_contracts::ai::runtime::ModelKind::FeatureWeighted,
                    handle: handle.clone(),
                    runtime: runtime.clone(),
                }) {
                    tracing::warn!(component="aicored", event="device_catalog_rejected", error=%error);
                } else {
                    model_binder = model_binder.with_device_catalog(Arc::new(catalog));
                }
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
        if let Some(simulator) = &simulated_platform {
            let exec = simulator.clone()
                as Arc<dyn aicore_contracts::traits::hardware_executor::IHardwareExecutor>;
            for matrix in simulator.hardware_matrices() {
                platform_binder = platform_binder.with_hardware(matrix.capabilities.clone());
            }
            platform_binder = platform_binder
                .with_gpu_executor(exec.clone())
                .with_npu_executor(exec);
        } else {
            #[cfg(target_os = "linux")]
            {
                use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;
                let exec = Arc::new(aicore_adapter_linux::LinuxHardwareExecutor::new())
                    as Arc<dyn aicore_contracts::traits::hardware_executor::IHardwareExecutor>;
                let hardware_telemetry =
                    aicore_adapter_linux::LinuxHardwareTelemetryProvider::new();
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
        }

        tracing::info!(
            component = "aicored",
            event = "runtime_binder_selected",
            binder = "default_runtime_binder",
            platform = if simulated_platform.is_some() {
                "simulated_platform"
            } else {
                "native"
            },
            "using selected platform runtime binding"
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

    let capabilities = simulated_platform
        .as_ref()
        .map(|platform| platform.capabilities_detailed())
        .unwrap_or_else(build_capabilities);
    let hardware_matrices = if let Some(platform) = &simulated_platform {
        platform.hardware_matrices().to_vec()
    } else if model_mode {
        Vec::new()
    } else {
        build_hardware_matrices()
    };

    /*
     * ============================================================
     * 14. Retention
     * ============================================================
     */

    let memory_store = memory_for_retention.clone();
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
    } else if let Some(platform) = &simulated_platform {
        let ids = platform.captured_process_ids();
        ExecutionTargets {
            foreground_pids: ids.first().copied().into_iter().collect(),
            background_pids: ids.iter().skip(1).copied().take(1).collect(),
            process_only: false,
        }
    } else {
        ExecutionTargets::default()
    };

    /*
     * ============================================================
     * 16. Final Runtime
     * ============================================================
     */

    DaemonRuntime {
        loop_: Arc::new(loop_),
        model_mode,
        simulated_platform: simulated_platform.is_some(),
        #[cfg(test)]
        simulator: simulated_platform,
        platform_name,
        capabilities,
        retention,
        l1_source,
        l2_provider,
        execution_targets,
        telemetry_selection,
        process_selection,
        hardware_matrices,
        memory_store,
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

type PlatformComponents = (
    Arc<dyn ITelemetryProvider>,
    Arc<ProcessSelection>,
    Arc<dyn IPolicyExecutor>,
    Arc<dyn IPostConditionChecker>,
    &'static str,
);

fn build_simulated_components(platform: Arc<SimulatedPlatform>) -> PlatformComponents {
    let telemetry: Arc<dyn ITelemetryProvider> = platform.clone();
    let process: Arc<dyn IProcessController> = platform.clone();
    let process_selection = Arc::new(ProcessSelection::new(process.clone()));
    let power: Arc<dyn IPowerController> = platform;
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(process_selection.clone()),
        Some(power.clone()),
    ));
    let checker: Arc<dyn IPostConditionChecker> = Arc::new(PlatformPostConditionChecker::new(
        Some(process),
        Some(power),
    ));
    (
        telemetry,
        process_selection,
        executor,
        checker,
        "simulated_platform",
    )
}

fn simulated_platform_mode() -> bool {
    std::env::var("AICORE_EXECUTION_MODE")
        .map(|value| value.eq_ignore_ascii_case("simulated-platform"))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn bundled_platform_fixture_json() -> &'static str {
    include_str!("../../../adapters/simulated-platform/fixtures/linux_wsl_readonly.json")
}

#[cfg(target_os = "windows")]
fn bundled_platform_fixture_json() -> &'static str {
    include_str!("../../../adapters/simulated-platform/fixtures/windows_readonly.json")
}

#[cfg(target_os = "linux")]
fn build_platform_components() -> PlatformComponents {
    use aicore_adapter_linux::{
        LinuxPowerController, LinuxProcessController, LinuxTelemetryProvider,
    };

    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(LinuxTelemetryProvider::new());

    let process: Arc<dyn IProcessController> = Arc::new(LinuxProcessController::new());
    let process_selection = Arc::new(ProcessSelection::new(process.clone()));
    let power: Arc<dyn IPowerController> = Arc::new(LinuxPowerController::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(process_selection.clone()),
        Some(power.clone()),
    ));
    let checker: Arc<dyn IPostConditionChecker> = Arc::new(PlatformPostConditionChecker::new(
        Some(process),
        Some(power),
    ));

    (telemetry, process_selection, executor, checker, "linux")
}

#[cfg(target_os = "windows")]
fn build_platform_components() -> PlatformComponents {
    use aicore_adapter_windows::{
        WindowsPowerController, WindowsProcessController, WindowsTelemetryProvider,
    };

    let telemetry: Arc<dyn ITelemetryProvider> = Arc::new(WindowsTelemetryProvider::new());

    let process: Arc<dyn IProcessController> = Arc::new(WindowsProcessController::new());
    let process_selection = Arc::new(ProcessSelection::new(process.clone()));
    let power: Arc<dyn IPowerController> = Arc::new(WindowsPowerController::new());
    let executor: Arc<dyn IPolicyExecutor> = Arc::new(DefaultPolicyExecutor::new(
        Some(process_selection.clone()),
        Some(power.clone()),
    ));
    let checker: Arc<dyn IPostConditionChecker> = Arc::new(PlatformPostConditionChecker::new(
        Some(process),
        Some(power),
    ));

    (telemetry, process_selection, executor, checker, "windows")
}

#[cfg(target_os = "linux")]
fn build_capabilities() -> Vec<CapabilityEntry> {
    use aicore_adapter_linux::LinuxCapabilityProvider;

    LinuxCapabilityProvider::new().capabilities_detailed()
}

#[cfg(target_os = "windows")]
fn build_capabilities() -> Vec<CapabilityEntry> {
    use aicore_adapter_windows::WindowsCapabilityProvider;

    WindowsCapabilityProvider::new().capabilities_detailed()
}

#[cfg(target_os = "linux")]
fn build_hardware_matrices() -> Vec<DeviceCapabilityMatrix> {
    use aicore_adapter_linux::{LinuxHardwareCapabilityProvider, LinuxHardwareTelemetryProvider};
    use aicore_contracts::{
        hardware::matrix::HostOs, traits::hardware_telemetry::IHardwareTelemetryProvider,
    };
    let telemetry = LinuxHardwareTelemetryProvider::new();
    let capabilities = LinuxHardwareCapabilityProvider::new();
    telemetry
        .gpu_devices()
        .into_iter()
        .filter_map(|device| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Linux,
                &device,
                capabilities.gpu_capability(&device.id),
            )
        })
        .chain(telemetry.npu_devices().into_iter().filter_map(|device| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Linux,
                &device,
                capabilities.npu_capability(&device.id),
            )
        }))
        .collect()
}

#[cfg(target_os = "windows")]
fn build_hardware_matrices() -> Vec<DeviceCapabilityMatrix> {
    use aicore_adapter_windows::{
        WindowsHardwareCapabilityProvider, WindowsHardwareTelemetryProvider,
    };
    use aicore_contracts::{
        hardware::matrix::HostOs, traits::hardware_telemetry::IHardwareTelemetryProvider,
    };
    let telemetry = WindowsHardwareTelemetryProvider::new();
    let capabilities = WindowsHardwareCapabilityProvider::new();
    telemetry
        .gpu_devices()
        .into_iter()
        .filter_map(|device| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Windows,
                &device,
                capabilities.gpu_capability(&device.id),
            )
        })
        .chain(telemetry.npu_devices().into_iter().filter_map(|device| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Windows,
                &device,
                capabilities.npu_capability(&device.id),
            )
        }))
        .collect()
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
/// AICORE_EXECUTION_MODE=simulated-platform
fn model_execution_mode() -> bool {
    match std::env::var("AICORE_EXECUTION_MODE") {
        Ok(value) if value.eq_ignore_ascii_case("simulated-platform") => false,
        Ok(value) if value.eq_ignore_ascii_case("platform") => false,

        Ok(value) if value.eq_ignore_ascii_case("native") => false,

        Ok(value) if value.eq_ignore_ascii_case("model") => true,

        _ => true,
    }
}

#[cfg(test)]
mod telemetry_selection_tests {
    use super::*;
    use aicore_contracts::{
        errors::{ContractError, ContractResult},
        telemetry::snapshot::ComputerSnapshot,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct IsolatedTelemetry {
        value: f32,
        fail: bool,
        calls: AtomicUsize,
    }
    impl ITelemetryProvider for IsolatedTelemetry {
        fn collect(&self) -> ContractResult<ComputerSnapshot> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(ContractError::AdapterUnavailable(
                    "mock plugin failed".into(),
                ));
            }
            let mut snapshot = ComputerSnapshot::default();
            snapshot.cpu.usage_percent = self.value;
            Ok(snapshot)
        }
        fn platform_name(&self) -> &'static str {
            "isolated"
        }
        fn capabilities(&self) -> Vec<&'static str> {
            Vec::new()
        }
    }

    #[test]
    fn typed_plugin_source_is_selected_then_fail_closed_to_platform_observer() {
        let fallback = Arc::new(IsolatedTelemetry {
            value: 10.0,
            fail: false,
            calls: AtomicUsize::new(0),
        });
        let selected = Arc::new(IsolatedTelemetry {
            value: 90.0,
            fail: false,
            calls: AtomicUsize::new(0),
        });
        let selection = TelemetrySelection::new(fallback.clone());
        selection.bind(selected);
        assert_eq!(selection.collect().unwrap().cpu.usage_percent, 90.0);
        let failed = Arc::new(IsolatedTelemetry {
            value: 0.0,
            fail: true,
            calls: AtomicUsize::new(0),
        });
        selection.bind(failed.clone());
        assert_eq!(selection.collect().unwrap().cpu.usage_percent, 10.0);
        assert_eq!(selection.collect().unwrap().cpu.usage_percent, 10.0);
        assert_eq!(failed.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fallback.calls.load(Ordering::SeqCst), 2);
    }
}

#[cfg(test)]
mod process_selection_tests {
    use super::*;
    use aicore_contracts::{
        errors::{ContractError, ContractResult},
        policy::priority::PolicyPriority,
        traits::process_controller::AffinityMask,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct IsolatedProcess {
        writes: AtomicUsize,
        fail: bool,
    }

    impl IProcessController for IsolatedProcess {
        fn set_priority(&self, _: u32, _: PolicyPriority) -> ContractResult<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(ContractError::AdapterUnavailable(
                    "mock plugin failed".into(),
                ))
            } else {
                Ok(())
            }
        }

        fn get_priority(&self, _: u32) -> ContractResult<PolicyPriority> {
            Ok(PolicyPriority::Normal)
        }

        fn set_affinity(&self, _: u32, _: &AffinityMask) -> ContractResult<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn get_affinity(&self, _: u32) -> ContractResult<AffinityMask> {
            Ok(AffinityMask::new(vec![0]))
        }

        fn capabilities(&self) -> Vec<&'static str> {
            vec!["process.priority"]
        }

        fn platform_name(&self) -> &'static str {
            "mock_process"
        }
    }

    #[test]
    fn selected_control_failure_never_falls_back_to_native_write() {
        let native = Arc::new(IsolatedProcess {
            writes: AtomicUsize::new(0),
            fail: false,
        });
        let plugin = Arc::new(IsolatedProcess {
            writes: AtomicUsize::new(0),
            fail: true,
        });
        let selection = ProcessSelection::new(native.clone());
        selection.bind(plugin.clone()).unwrap();
        assert!(selection.set_priority(42, PolicyPriority::High).is_err());
        assert_eq!(plugin.writes.load(Ordering::SeqCst), 1);
        assert_eq!(native.writes.load(Ordering::SeqCst), 0);
        assert_eq!(selection.get_priority(42).unwrap(), PolicyPriority::Normal);
        assert!(selection
            .set_affinity(42, &AffinityMask::new(vec![0]))
            .is_err());
        assert_eq!(native.writes.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
mod simulated_platform_tests {
    use super::*;
    use aicore_contracts::{
        execution::status::ExecutionOverallStatus,
        policy::{priority::PolicyPriority, resource_policy::ResourcePolicy},
        runtime::mode::RuntimeMode,
        telemetry::snapshot::ComputerSnapshot,
        traits::{
            process_controller::IProcessController, runtime_orchestrator::IRuntimeOrchestrator,
            transparent_executor::ITransparentExecutor,
        },
    };
    use aicore_runtime::dual_loop::PlatformTickDriver;
    use std::time::{Duration, Instant};

    fn fixture() -> Arc<SimulatedPlatform> {
        Arc::new(SimulatedPlatform::from_json(bundled_platform_fixture_json()).unwrap())
    }

    #[test]
    fn captured_fixture_is_used_by_all_simulated_components() {
        let simulator = fixture();
        assert!(!simulator.captured_process_ids().is_empty());
        let (telemetry, _, _, _, platform_name) = build_simulated_components(simulator.clone());
        assert_eq!(platform_name, "simulated_platform");
        let sample = telemetry.collect().unwrap();
        assert!(sample.collected_at_unix_ms.is_some());
        let pid = simulator.captured_process_ids()[0];
        let original = simulator.get_priority(pid).unwrap();
        let next = if original == PolicyPriority::High {
            PolicyPriority::Normal
        } else {
            PolicyPriority::High
        };
        simulator.set_priority(pid, next).unwrap();
        assert_eq!(simulator.get_priority(pid).unwrap(), next);
        simulator.set_priority(pid, original).unwrap();
        assert_eq!(simulator.get_priority(pid).unwrap(), original);
        assert!(simulator.set_priority(u32::MAX, next).is_err());
    }

    #[test]
    fn simulated_readback_failure_rolls_back_without_claiming_success() {
        let simulator = fixture();
        let pid = simulator.captured_process_ids()[0];
        let original = simulator.get_priority(pid).unwrap();
        let next = if original == PolicyPriority::High {
            PolicyPriority::Normal
        } else {
            PolicyPriority::High
        };
        let (_, _, executor, checker, _) = build_simulated_components(simulator.clone());
        let transparent = TransparentExecutor::new(executor).with_checker(checker);
        let previous = ResourcePolicy {
            policy_id: "prior".into(),
            cpu: aicore_contracts::policy::resource_policy::CpuPolicy {
                priority: original,
                ..Default::default()
            },
            ..Default::default()
        };
        let proposed = ResourcePolicy {
            policy_id: "proposed".into(),
            cpu: aicore_contracts::policy::resource_policy::CpuPolicy {
                priority: next,
                ..Default::default()
            },
            ..Default::default()
        };
        let targets = ExecutionTargets {
            foreground_pids: vec![pid],
            background_pids: Vec::new(),
            process_only: false,
        };
        simulator.stale_read_after_next_write().unwrap();
        let trace = transparent.execute_transparent(&proposed, &targets, Some(&previous));
        assert!(trace.rollback.triggered);
        assert_eq!(trace.rollback.succeeded, Some(true));
        assert_eq!(trace.final_status, ExecutionOverallStatus::NoOp);
        assert_eq!(simulator.get_priority(pid).unwrap(), original);
    }

    struct DelayedCapturedTelemetry {
        simulator: Arc<SimulatedPlatform>,
        delay: Duration,
    }
    impl ITelemetryProvider for DelayedCapturedTelemetry {
        fn collect(&self) -> aicore_contracts::errors::ContractResult<ComputerSnapshot> {
            std::thread::sleep(self.delay);
            self.simulator.collect()
        }
        fn platform_name(&self) -> &'static str {
            "delayed_captured_simulator"
        }
        fn capabilities(&self) -> Vec<&'static str> {
            vec!["cpu.telemetry"]
        }
    }

    #[test]
    fn daemon_platform_driver_deadline_and_fresh_apply_use_simulator() {
        let runtime = build_runtime_for_mode_with_simulation(None, None, false, true);
        assert!(runtime.simulated_platform);
        assert!(!runtime.model_mode);
        assert_eq!(runtime.platform_name, "simulated_platform");
        let simulator = runtime.simulator.as_ref().unwrap().clone();
        assert!(!runtime.execution_targets.foreground_pids.is_empty());
        runtime
            .telemetry_selection
            .bind(Arc::new(DelayedCapturedTelemetry {
                simulator: simulator.clone(),
                delay: Duration::from_millis(250),
            }));
        let driver = PlatformTickDriver::new(
            runtime.loop_.clone(),
            runtime.execution_targets.clone(),
            Duration::from_millis(50),
        )
        .unwrap();
        let start = Instant::now();
        let first = driver.tick();
        assert_eq!(first.execution_status, Some(ExecutionOverallStatus::NoOp));
        assert!(start.elapsed() < Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(280));
        assert_eq!(
            driver.tick().execution_status,
            Some(ExecutionOverallStatus::NoOp)
        );
        assert_eq!(
            runtime.loop_.safe_mode().current_mode(),
            RuntimeMode::Normal
        );

        let fresh = build_runtime_for_mode_with_simulation(None, None, false, true);
        let fresh_simulator = fresh.simulator.as_ref().unwrap().clone();
        let pid = fresh.execution_targets.foreground_pids[0];
        let fresh_driver = PlatformTickDriver::new(
            fresh.loop_.clone(),
            fresh.execution_targets.clone(),
            Duration::from_millis(500),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let outcome = fresh_driver.tick();
            if outcome.validation_accepted {
                assert!(matches!(
                    outcome.execution_status,
                    Some(
                        ExecutionOverallStatus::AllSucceeded
                            | ExecutionOverallStatus::PartiallySucceeded
                    )
                ));
                let policy_json = outcome.current_policy_json.unwrap();
                let policy: ResourcePolicy = serde_json::from_str(&policy_json).unwrap();
                assert_eq!(
                    fresh_simulator.get_priority(pid).unwrap(),
                    policy.cpu.priority
                );
                assert_eq!(fresh.loop_.safe_mode().current_mode(), RuntimeMode::Normal);
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("simulated Platform Mode did not consume a fresh prepared policy");
    }

    #[test]
    fn repeated_simulator_execution_failure_enters_safe_mode() {
        let runtime = build_runtime_for_mode_with_simulation(None, None, false, true);
        let simulator = runtime.simulator.as_ref().unwrap();
        assert!(!runtime.execution_targets.foreground_pids.is_empty());
        for _ in 0..5 {
            simulator.fail_next_process_write().unwrap();
            let outcome = runtime.loop_.tick(&runtime.execution_targets);
            assert_eq!(
                outcome.execution_status,
                Some(ExecutionOverallStatus::AllFailed)
            );
        }
        assert_eq!(runtime.loop_.safe_mode().current_mode(), RuntimeMode::Safe);
    }
}
