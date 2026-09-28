use crate::application::id::ApplicationId;
use crate::context::snapshot::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::telemetry::snapshot::ComputerSnapshot;

/// Projector input.
///
/// Pure data, does not contain trip object.
#[derive(Debug, Clone, Default)]
pub struct PredictionInput {
    pub application_id: Option<ApplicationId>,
    pub snapshot: ComputerSnapshot,
    pub context: ContextSnapshot,
    /// Optional history following; None without memory.
    pub memory: Option<MemoryContext>,
    /// Forecast window, millisecond.
    pub horizon_ms: u64,
}
