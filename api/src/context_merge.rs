use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    task::{preferences::LatencySensitivity, TaskContract},
};

/// 将 TaskContract 覆盖到基础 ContextSnapshot 上。
///
/// 语义：
/// - 应用显式声明的字段覆盖 ContextEngine 的推断。
/// - 未声明的字段保留 ContextEngine 结果。
/// - 覆盖时 confidence 提升到 1.0，表示"应用显式声明"。
pub fn apply_task_contract(base: &ContextSnapshot, contract: &TaskContract) -> ContextSnapshot {
    let mut merged = base.clone();
    let mut overridden = false;

    if let Some(w) = contract.task {
        merged.workload_type = w;
        overridden = true;
    } else {
        // 若未显式声明，但应用类别可推断，保持 base
        let _ = WorkloadType::Unknown;
    }

    if let Some(latency) = contract.latency {
        merged.interaction_level = match latency {
            LatencySensitivity::Critical | LatencySensitivity::High => {
                InteractionLevel::Interactive
            },
            LatencySensitivity::Medium => InteractionLevel::Passive,
            LatencySensitivity::Low => InteractionLevel::Idle,
        };
        overridden = true;
    }

    if overridden {
        merged.confidence = 1.0;
    }

    merged
}
