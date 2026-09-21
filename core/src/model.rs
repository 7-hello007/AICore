use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use aicore_contracts::{
    execution::{
        result::{ExecutionOutcome, ExecutionResult},
        status::{ExecutionOverallStatus, ExecutionStatus},
        targets::ExecutionTargets,
    },
    policy::{
        priority::PolicyPriority,
        resource_policy::{PowerMode, ResourcePolicy},
    },
    traits::{
        policy_executor::IPolicyExecutor,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
    },
};

/// 模型环境中的虚拟前台进程。
///
/// 这些 PID 只存在于模型状态中，绝不能传给真实 OS ProcessController。
pub const MODEL_FOREGROUND_PID: u32 = 10_001;

/// 模型环境中的虚拟后台进程。
pub const MODEL_BACKGROUND_PID: u32 = 10_002;

/// 模拟进程。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProcessState {
    pub pid: u32,
    pub name: String,
    pub priority: PolicyPriority,
    pub affinity_cores: Vec<u32>,
    pub throttled: bool,
    pub max_cpu_percent: Option<f32>,
}

impl ModelProcessState {
    pub fn new(pid: u32, name: impl Into<String>, priority: PolicyPriority) -> Self {
        Self {
            pid,
            name: name.into(),
            priority,
            affinity_cores: Vec::new(),
            throttled: false,
            max_cpu_percent: None,
        }
    }
}

/// 模型系统状态。
///
/// 所有策略执行只修改这里，不修改宿主机。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSystemState {
    pub power_mode: PowerMode,

    pub gpu_workload_preference: Option<String>,

    pub npu_preferred: bool,

    pub memory_max_usage_percent: Option<f32>,

    pub processes: BTreeMap<u32, ModelProcessState>,

    pub last_policy_id: Option<String>,

    pub revision: u64,
}

impl Default for ModelSystemState {
    fn default() -> Self {
        let mut processes = BTreeMap::new();

        processes.insert(
            MODEL_FOREGROUND_PID,
            ModelProcessState::new(
                MODEL_FOREGROUND_PID,
                "model-foreground-app",
                PolicyPriority::Normal,
            ),
        );

        processes.insert(
            MODEL_BACKGROUND_PID,
            ModelProcessState::new(
                MODEL_BACKGROUND_PID,
                "model-background-worker",
                PolicyPriority::Background,
            ),
        );

        Self {
            power_mode: PowerMode::Balanced,
            gpu_workload_preference: None,
            npu_preferred: false,
            memory_max_usage_percent: None,
            processes,
            last_policy_id: None,
            revision: 0,
        }
    }
}

/// 模型执行环境。
///
/// 它是整个 Model Execution Layer 的状态容器。
///
/// 重要：
/// - 不访问 PID。
/// - 不访问 /proc。
/// - 不写 sysfs。
/// - 不调用真实 GPU / NPU API。
/// - 不修改系统电源配置。
#[derive(Clone)]
pub struct ModelEnvironment {
    state: Arc<Mutex<ModelSystemState>>,
}

impl ModelEnvironment {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Arc::new(Mutex::new(ModelSystemState::default())),
        })
    }

    pub fn snapshot(&self) -> ModelSystemState {
        self.state
            .lock()
            .expect("model environment lock poisoned")
            .clone()
    }

    fn update<F>(&self, update: F)
    where
        F: FnOnce(&mut ModelSystemState),
    {
        let mut state = self.state.lock().expect("model environment lock poisoned");

        update(&mut state);
    }

    fn ensure_process<'a>(
        state: &'a mut ModelSystemState,
        pid: u32,
        name: &str,
    ) -> &'a mut ModelProcessState {
        state
            .processes
            .entry(pid)
            .or_insert_with(|| ModelProcessState::new(pid, name, PolicyPriority::Normal))
    }
}

/// 模型目标提供器。
///
/// 这里提供的是“虚拟 PID”，仅供模型执行器使用。
///
/// 这样 AICore 可以拥有完整的：
///
/// Policy
///   ↓
/// ExecutionTargets
///   ↓
/// Executor
///
/// 链路，同时不会碰真实系统进程。
#[derive(Debug, Clone)]
pub struct ModelTargetProvider {
    foreground_pids: Vec<u32>,
    background_pids: Vec<u32>,
}

impl Default for ModelTargetProvider {
    fn default() -> Self {
        Self {
            foreground_pids: vec![MODEL_FOREGROUND_PID],
            background_pids: vec![MODEL_BACKGROUND_PID],
        }
    }
}

impl ModelTargetProvider {
    pub fn new(foreground_pids: Vec<u32>, background_pids: Vec<u32>) -> Self {
        Self {
            foreground_pids,
            background_pids,
        }
    }

    pub fn execution_targets(&self) -> ExecutionTargets {
        ExecutionTargets {
            foreground_pids: self.foreground_pids.clone(),
            background_pids: self.background_pids.clone(),
        }
    }
}

/// 模型策略执行器。
///
/// 这是 `DefaultPolicyExecutor` 的模型替代版本。
///
/// 与真实执行器的区别：
/// - 不调用 IProcessController。
/// - 不调用 IPowerController。
/// - 不修改真实系统。
/// - 所有资源状态写入 ModelEnvironment。
pub struct ModelPolicyExecutor {
    environment: Arc<ModelEnvironment>,
}

impl ModelPolicyExecutor {
    pub fn new(environment: Arc<ModelEnvironment>) -> Self {
        Self { environment }
    }

    pub fn environment(&self) -> Arc<ModelEnvironment> {
        self.environment.clone()
    }

    fn effective_targets(&self, targets: &ExecutionTargets) -> ExecutionTargets {
        // 如果上层没有提供 targets，模型层仍然使用自己的虚拟默认目标。
        //
        // 这样可以保证：
        // runtime.tick(&ExecutionTargets::default())
        // 在模型模式下仍然产生真实 ExecutionOutcome。
        if targets.foreground_pids.is_empty() && targets.background_pids.is_empty() {
            return ModelTargetProvider::default().execution_targets();
        }

        targets.clone()
    }
}

impl IPolicyExecutor for ModelPolicyExecutor {
    fn execute(&self, policy: &ResourcePolicy, targets: &ExecutionTargets) -> ExecutionResult {
        let started_at_unix_ms = now_unix_ms();

        let effective_targets = self.effective_targets(targets);

        let mut outcomes: Vec<ExecutionOutcome> = Vec::new();

        self.environment.update(|state| {
            /*
             * 1. Power
             */
            state.power_mode = policy.power.mode;

            outcomes.push(success(
                "model.power.profile",
                "model-system",
                format!("simulated power mode = {:?}", policy.power.mode),
            ));

            /*
             * 2. Foreground process priority
             */
            for pid in &effective_targets.foreground_pids {
                let process = ModelEnvironment::ensure_process(
                    state,
                    *pid,
                    &format!("model-foreground-{}", pid),
                );

                process.priority = policy.cpu.priority;
                process.throttled = false;
                process.max_cpu_percent = None;

                outcomes.push(success(
                    "model.process.priority",
                    &format!("pid={}", pid),
                    format!("simulated foreground priority = {:?}", policy.cpu.priority),
                ));
            }

            /*
             * 3. CPU affinity
             */
            if let Some(cores) = &policy.cpu.affinity_cores {
                for pid in effective_targets
                    .foreground_pids
                    .iter()
                    .chain(effective_targets.background_pids.iter())
                {
                    let process = ModelEnvironment::ensure_process(
                        state,
                        *pid,
                        &format!("model-process-{}", pid),
                    );

                    process.affinity_cores = cores.clone();

                    outcomes.push(success(
                        "model.process.affinity",
                        &format!("pid={}", pid),
                        format!("simulated affinity cores = {:?}", cores),
                    ));
                }
            }

            /*
             * 4. Background throttling
             */
            if policy.background.throttle {
                for pid in &effective_targets.background_pids {
                    if effective_targets.foreground_pids.contains(pid) {
                        continue;
                    }

                    let process = ModelEnvironment::ensure_process(
                        state,
                        *pid,
                        &format!("model-background-{}", pid),
                    );

                    process.priority = PolicyPriority::Background;
                    process.throttled = true;
                    process.max_cpu_percent = policy.background.max_cpu_percent;

                    outcomes.push(success(
                        "model.process.throttle",
                        &format!("pid={}", pid),
                        format!(
                            "simulated background throttle max_cpu={:?}",
                            policy.background.max_cpu_percent
                        ),
                    ));
                }
            } else {
                for pid in &effective_targets.background_pids {
                    if effective_targets.foreground_pids.contains(pid) {
                        continue;
                    }

                    let process = ModelEnvironment::ensure_process(
                        state,
                        *pid,
                        &format!("model-background-{}", pid),
                    );

                    process.throttled = false;
                    process.max_cpu_percent = None;
                }
            }

            /*
             * 5. GPU policy
             */
            state.gpu_workload_preference = policy.gpu.workload_preference.clone();

            if let Some(preference) = &policy.gpu.workload_preference {
                outcomes.push(success(
                    "model.gpu.preference",
                    "model-gpu",
                    format!("simulated GPU workload preference = {}", preference),
                ));
            } else {
                outcomes.push(success(
                    "model.gpu.preference",
                    "model-gpu",
                    "no explicit GPU preference",
                ));
            }

            /*
             * 6. NPU policy
             */
            state.npu_preferred = policy.npu.preferred;

            outcomes.push(success(
                "model.npu.preference",
                "model-npu",
                format!("simulated NPU preferred = {}", policy.npu.preferred),
            ));

            /*
             * 7. Memory policy
             */
            state.memory_max_usage_percent = policy.memory.max_usage_percent;

            if let Some(limit) = policy.memory.max_usage_percent {
                outcomes.push(success(
                    "model.memory.limit",
                    "model-memory",
                    format!("simulated memory limit = {:.1}%", limit),
                ));
            } else {
                outcomes.push(success(
                    "model.memory.limit",
                    "model-memory",
                    "no explicit memory limit",
                ));
            }

            /*
             * 8. Commit model policy
             */
            state.last_policy_id = Some(policy.policy_id.clone());
            state.revision = state.revision.saturating_add(1);

            outcomes.push(success(
                "model.policy.commit",
                "model-state",
                format!("model policy committed at revision {}", state.revision),
            ));
        });

        let overall_status = ExecutionOverallStatus::from_outcomes(&outcomes);

        let finished_at_unix_ms = now_unix_ms();

        tracing::info!(
            component = "model_executor",
            event = "model_policy_applied",
            policy_id = %policy.policy_id,
            foreground_targets = effective_targets.foreground_pids.len(),
            background_targets = effective_targets.background_pids.len(),
            outcomes = outcomes.len(),
            overall_status = ?overall_status,
            "model policy applied without host side effects"
        );

        ExecutionResult {
            policy_id: policy.policy_id.clone(),
            overall_status,
            started_at_unix_ms,
            finished_at_unix_ms,
            outcomes,
        }
    }

    fn name(&self) -> &'static str {
        "model_policy_executor"
    }
}

/// 模型后置条件检查器。
///
/// TransparentExecutor 的 Verify 阶段会调用它。
///
/// 这样模型链不是：
/// Apply → Skipped Verify
///
/// 而是：
/// Apply → Verify Passed
pub struct ModelPostConditionChecker {
    environment: Arc<ModelEnvironment>,
}

impl ModelPostConditionChecker {
    pub fn new(environment: Arc<ModelEnvironment>) -> Self {
        Self { environment }
    }
}

impl IPostConditionChecker for ModelPostConditionChecker {
    fn verify(&self, policy: &ResourcePolicy, result: &ExecutionResult) -> PostConditionResult {
        if !matches!(
            result.overall_status,
            ExecutionOverallStatus::AllSucceeded | ExecutionOverallStatus::PartiallySucceeded
        ) {
            return PostConditionResult::Failed(format!(
                "model apply status was {:?}",
                result.overall_status
            ));
        }

        let state = self.environment.snapshot();

        if state.last_policy_id.as_deref() != Some(policy.policy_id.as_str()) {
            return PostConditionResult::Failed("model state policy id mismatch".to_string());
        }

        if state.power_mode != policy.power.mode {
            return PostConditionResult::Failed(format!(
                "model power mode mismatch: expected={:?} actual={:?}",
                policy.power.mode, state.power_mode
            ));
        }

        if state.npu_preferred != policy.npu.preferred {
            return PostConditionResult::Failed(format!(
                "model NPU preference mismatch: expected={} actual={}",
                policy.npu.preferred, state.npu_preferred
            ));
        }

        if state.gpu_workload_preference != policy.gpu.workload_preference {
            return PostConditionResult::Failed(
                "model GPU workload preference mismatch".to_string(),
            );
        }

        if state.memory_max_usage_percent != policy.memory.max_usage_percent {
            return PostConditionResult::Failed("model memory policy mismatch".to_string());
        }

        PostConditionResult::Passed
    }

    fn checker_name(&self) -> &'static str {
        "model_post_condition_checker"
    }
}

fn success(resource: &str, target: &str, message: impl Into<String>) -> ExecutionOutcome {
    ExecutionOutcome {
        resource: resource.to_string(),
        target: target.to_string(),
        status: ExecutionStatus::Success,
        message: message.into(),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
