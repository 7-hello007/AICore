use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    task::{preferences::LatencySensitivity, TaskContract},
};

/// Overwrite TaskContract over the base ContextSnapshot.
///
/// Semantics:
/// - Apply the field of the explicit declaration to the ContextEngine extrapolation.
/// - Undeclared fields retain ContextEngine results.
/// - Override confidence up to 1.0, meaning "apply a explicit declaration".
pub fn apply_task_contract(base: &ContextSnapshot, contract: &TaskContract) -> ContextSnapshot {
    let mut merged = base.clone();
    let mut overridden = false;

    if let Some(w) = contract.task {
        merged.workload_type = w;
        overridden = true;
    } else {
        // If no explicit declaration is available, but application categories can be extrapolated, base maintained
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
