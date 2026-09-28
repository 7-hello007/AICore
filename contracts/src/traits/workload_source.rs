use crate::context::workload::WorkloadType;
use crate::errors::ContractResult;
use crate::telemetry::ComputerSnapshot;

/// Workload observation.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadObservation {
    pub workload_type: WorkloadType,
    /// Confidence, 0.0-1.0.
    pub confidence: f32,
    /// A summary of human-readable evidence.
    pub evidence: Vec<String>,
}

/// Semantic categories of observers.
///
/// For Fusion to decide "Rules First" or "ML Plus".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Rule / Determination logic.
    Rule,
    /// Machine Learning / Statistical Model.
    Ml,
    /// Application of an explicit declaration (L1/L.2).
    Declared,
}

/// Worldload observation source.
///
/// This can be implemented by L0 rules extrapolation, ML classification, L1 declaration, L2 native provider.
/// Core obtains workload via this interface; does not care about the specific Tier.
pub trait IWorkloadSource: Send + Sync {
    fn observe(&self, snapshot: &ComputerSnapshot) -> ContractResult<WorkloadObservation>;

    /// The name of the provider, for the log.
    fn source_name(&self) -> &'static str;

    /// Priority; higher value.
    fn priority(&self) -> u32 {
        100
    }

    /// Semantic Category. Default Rule.
    fn kind(&self) -> SourceKind {
        SourceKind::Rule
    }

    /// It comes from my declared confidence ceiling.
    ///
    /// Fusion uses it to judge whether it can be directly adopted.
    fn confidence_ceiling(&self) -> f32 {
        1.0
    }
}
