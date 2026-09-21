use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::policy_engine::IPolicyEngine,
};

/// 基于规则的策略引擎。
///
/// 设计目标：
/// - 确定性：同样输入 → 同样输出（除 policy_id / 时间戳外）。
/// - 可解释：reason 字段说明为什么这么选。
/// - 可替换：AI 建议器后续实现同一接口。
pub struct RuleBasedPolicyEngine;

impl RuleBasedPolicyEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RuleBasedPolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl IPolicyEngine for RuleBasedPolicyEngine {
    fn generate(&self, snapshot: &ComputerSnapshot, context: &ContextSnapshot) -> ResourcePolicy {
        let base = base_policy_for(context.workload_type);
        let (cpu, gpu, npu, mem, mut power_mode, throttle) = base;

        // Interaction level 调整
        let (cpu, gpu, npu, mem) = match context.interaction_level {
            InteractionLevel::Interactive => (cpu, gpu, npu, mem),
            InteractionLevel::Passive => (
                downgrade(cpu),
                downgrade(gpu),
                downgrade(npu),
                downgrade(mem),
            ),
            InteractionLevel::Idle => (
                PolicyPriority::Low,
                PolicyPriority::Low,
                PolicyPriority::Low,
                PolicyPriority::Low,
            ),
        };

        // 电源感知：电池供电且未充电时，非重度 workload 优先省电
        let on_battery = snapshot.battery.present
            && !snapshot.battery.charging.unwrap_or(true)
            && !snapshot.power.ac_connected;

        if on_battery {
            let heavy = matches!(
                context.workload_type,
                WorkloadType::Gaming
                    | WorkloadType::VideoEditing
                    | WorkloadType::Rendering
                    | WorkloadType::AiInference
            );
            power_mode = if heavy {
                PowerMode::Balanced
            } else {
                PowerMode::PowerSave
            };
        }

        // 整体优先级 = 所有子资源的最大值，满足 Validator 不变量
        let overall = [cpu, gpu, npu, mem]
            .into_iter()
            .max()
            .unwrap_or(PolicyPriority::Normal);

        // NPU preferred 只在可用且优先级 >= Normal 时启用
        let npu_preferred = snapshot.npu.available && npu >= PolicyPriority::Normal;

        let reason = format!(
            "workload={:?} interaction={:?} on_battery={} source=rule_based",
            context.workload_type, context.interaction_level, on_battery
        );

        ResourcePolicy {
            policy_id: generate_policy_id(),
            task_id: None,
            priority: overall,
            source: PolicySource::RuleBased,
            reason,
            cpu: CpuPolicy {
                priority: cpu,
                affinity_cores: None,
            },
            gpu: GpuPolicy {
                priority: gpu,
                workload_preference: None,
            },
            npu: NpuPolicy {
                priority: npu,
                preferred: npu_preferred,
            },
            memory: MemoryPolicy {
                priority: mem,
                max_usage_percent: None,
            },
            power: PowerPolicy { mode: power_mode },
            background: BackgroundPolicy {
                throttle,
                max_cpu_percent: if throttle { Some(20.0) } else { None },
            },
            created_at_unix_ms: now_unix_ms(),
        }
    }

    fn name(&self) -> &'static str {
        "rule_based_policy_engine"
    }
}

type BasePolicy = (
    PolicyPriority,
    PolicyPriority,
    PolicyPriority,
    PolicyPriority,
    PowerMode,
    bool,
);

fn base_policy_for(workload: WorkloadType) -> BasePolicy {
    use PolicyPriority::*;
    match workload {
        WorkloadType::Development => (High, Normal, Normal, Normal, PowerMode::Balanced, true),
        WorkloadType::Gaming => (High, High, Low, High, PowerMode::Performance, true),
        WorkloadType::VideoEditing => (High, High, Low, High, PowerMode::Performance, true),
        WorkloadType::AiInference => (Normal, High, High, High, PowerMode::Performance, true),
        WorkloadType::Rendering => (High, High, Low, High, PowerMode::Performance, true),
        WorkloadType::Compilation => (High, Low, Low, Normal, PowerMode::Performance, true),
        WorkloadType::FileProcessing => (Normal, Low, Low, Normal, PowerMode::Balanced, true),
        WorkloadType::VideoConference => (High, Normal, Low, Normal, PowerMode::Balanced, true),
        WorkloadType::GeneralOffice => (Normal, Low, Low, Normal, PowerMode::Balanced, false),
        WorkloadType::BackgroundTask => (Low, Low, Low, Low, PowerMode::PowerSave, true),
        WorkloadType::Unknown => (Normal, Normal, Normal, Normal, PowerMode::Balanced, false),
    }
}

fn downgrade(p: PolicyPriority) -> PolicyPriority {
    match p {
        PolicyPriority::Interactive => PolicyPriority::High,
        PolicyPriority::High => PolicyPriority::Normal,
        PolicyPriority::Normal => PolicyPriority::Low,
        PolicyPriority::Low => PolicyPriority::Low,
        PolicyPriority::Background => PolicyPriority::Background,
    }
}

fn generate_policy_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("policy-{:x}", nanos)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
