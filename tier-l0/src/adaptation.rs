use std::sync::Arc;

use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    errors::ContractResult,
    policy::{priority::PolicyPriority, resource_policy::ResourcePolicy},
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        adaptation_provider::{AdaptationHint, IAdaptationProvider},
        policy_engine::IPolicyEngine,
    },
};
use aicore_core::policy::RuleBasedPolicyEngine;

/// L0 analysis projecter: a rule-based proposal to generate resource strategies from observed states.
///
/// Characteristics:
/// - No direct implementation of any controls; output only hint.
/// - It is suggested that the Core Validator and Execut will go along the same path.
/// - returns `None` with low confidence, allowing Core to maintain the current policy.
/// - **Capability-driving**: GPU/NPU not available to downgrade relevant fields with annotations.
pub struct L0AdaptationProvider {
    engine: Arc<dyn IPolicyEngine>,
    min_confidence: f32,
}

impl L0AdaptationProvider {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(RuleBasedPolicyEngine::new()),
            min_confidence: 0.5,
        }
    }

    pub fn with_engine(engine: Arc<dyn IPolicyEngine>) -> Self {
        Self {
            engine,
            min_confidence: 0.5,
        }
    }

    pub fn with_min_confidence(mut self, min: f32) -> Self {
        self.min_confidence = min.clamp(0.0, 1.0);
        self
    }

    pub fn min_confidence(&self) -> f32 {
        self.min_confidence
    }
}

impl Default for L0AdaptationProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IAdaptationProvider for L0AdaptationProvider {
    fn suggest(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<Option<AdaptationHint>> {
        // L0 handles only recognized scenes; Unknown workload maintains the original policy.
        if context.workload_type == WorkloadType::Unknown {
            return Ok(None);
        }

        let confidence = context.confidence.clamp(0.0, 1.0);
        if confidence < self.min_confidence {
            return Ok(None);
        }

        let base_policy = self.engine.generate(snapshot, context);

        // === Capability-driven degradation ===
        let gpu_available = snapshot.gpu.usage_percent.is_some();
        let npu_available = snapshot.npu.available;

        let mut policy = base_policy;
        let mut evidence: Vec<String> = Vec::new();
        evidence.push(format!("workload={:?}", context.workload_type));

        if !gpu_available {
            // No GPU telemetry does not claim to control GPU.
            policy.gpu.priority = PolicyPriority::Normal;
            evidence.push("gpu_unavailable: gpu policy downgraded".to_string());
        }
        if !npu_available {
            // No NPU → cleared preferred.
            policy.npu.priority = PolicyPriority::Normal;
            policy.npu.preferred = false;
            evidence.push("npu_unavailable: npu policy downgraded".to_string());
        }

        // No change: Overall > = all sub-resources.
        let overall = [
            policy.cpu.priority,
            policy.gpu.priority,
            policy.npu.priority,
            policy.memory.priority,
        ]
        .into_iter()
        .max()
        .unwrap_or(policy.priority);
        policy.priority = overall;

        let reason = format!("l0 rule-based adaptation: {}", evidence.join("; "));

        Ok(Some(AdaptationHint {
            policy_override: Some(policy),
            reason,
            confidence,
        }))
    }

    fn source_name(&self) -> &'static str {
        "l0.adaptation_provider"
    }

    fn priority(&self) -> u32 {
        // L0 Minimum priority: L1/L2 explicit declaration priority.
        100
    }
}

/// Empty policy check aids for testing.
#[allow(dead_code)]
pub(crate) fn policy_is_empty(p: &ResourcePolicy) -> bool {
    p.reason.trim().is_empty()
}
