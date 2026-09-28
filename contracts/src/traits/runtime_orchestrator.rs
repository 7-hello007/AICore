use crate::execution::targets::ExecutionTargets;
use crate::runtime::{metrics::LoopMetrics, mode::RuntimeMode, tick::TickOutcome};

/// .
///
/// ♪ Make sure ♪
/// Thread into a synchronized tick; dispatch (timer, thread) is the responsibility of the caller.
pub trait IRuntimeOrchestrator: Send + Sync {
    fn tick(&self, targets: &ExecutionTargets) -> TickOutcome;
    fn current_mode(&self) -> RuntimeMode;
    fn metrics(&self) -> LoopMetrics;
    fn name(&self) -> &'static str;
}
