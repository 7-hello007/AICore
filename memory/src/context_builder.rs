use aicore_contracts::memory::{context::MemoryContext, record::HistoricalRecord};

/// Build MemoryContext from historical records.
///
/// Entrusts `MemoryContext::from_records` to preserve this function to maintain backward compatibility.
pub fn build_memory_context(records: &[HistoricalRecord]) -> MemoryContext {
    MemoryContext::from_records(records)
}
