use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;

/// Runtime route source.
///
/// Semantics:
/// - `Native`: Do not change the default execution path (fallback default).
/// - `Cpu`: CPU execution.
/// - `Gpu`: GPU execution.
/// - `Npu`: NPU execution.
/// - `AiRuntime`_: adopted AICore Yes. `IAiRuntime` Implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteSource {
    Native,
    Cpu,
    Gpu,
    Npu,
    AiRuntime,
}

impl RuntimeRouteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Npu => "npu",
            Self::AiRuntime => "ai_runtime",
        }
    }

    /// Whether it's the "no-change application" path.
    pub fn is_native(self) -> bool {
        self == Self::Native
    }
}

/// An Runtime route decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeRoute {
    pub route_id: String,
    /// Decision against workload.
    pub workload_type: WorkloadType,
    /// Select the execution path.
    pub source: RuntimeRouteSource,
    /// Whether or not the route (false = maintenance) is really applied.
    pub applied: bool,
    /// Reason for decision-making.
    pub reason: String,
    /// Confidence, 0.0-1.0.
    pub confidence: f32,
    /// Decision time.
    pub decided_at_unix_secs: u64,
}

impl RuntimeRoute {
    pub fn native(workload: WorkloadType, reason: impl Into<String>) -> Self {
        Self {
            route_id: String::new(),
            workload_type: workload,
            source: RuntimeRouteSource::Native,
            applied: false,
            reason: reason.into(),
            confidence: 1.0,
            decided_at_unix_secs: 0,
        }
    }

    pub fn routed(
        workload: WorkloadType,
        source: RuntimeRouteSource,
        reason: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            route_id: String::new(),
            workload_type: workload,
            source,
            applied: true,
            reason: reason.into(),
            confidence: confidence.clamp(0.0, 1.0),
            decided_at_unix_secs: 0,
        }
    }

    pub fn is_native(&self) -> bool {
        self.source.is_native()
    }
}
