use crate::context::ContextSnapshot;
use crate::memory::context::MemoryContext;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// Enter AI Provider.
///
/// Design principles:
/// - Pure data, does not contain trip object.
/// - base_policy is optional, because AI may be used to "understand only the context".
/// -Memory is optional, because not all Provider consumes history.
#[derive(Debug, Clone, Default)]
pub struct AiInput {
    pub snapshot: ComputerSnapshot,
    pub context: ContextSnapshot,
    pub base_policy: Option<ResourcePolicy>,
    pub memory: Option<MemoryContext>,
}
