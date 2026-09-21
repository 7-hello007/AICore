use aicore_contracts::memory::{context::MemoryContext, record::HistoricalRecord};

/// 从历史记录构建 MemoryContext。
///
/// 委托给 `MemoryContext::from_records`，保留此函数以维持向后兼容。
pub fn build_memory_context(records: &[HistoricalRecord]) -> MemoryContext {
    MemoryContext::from_records(records)
}
