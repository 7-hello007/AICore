use crate::execution::targets::ExecutionTargets;
use crate::runtime::{metrics::LoopMetrics, mode::RuntimeMode, tick::TickOutcome};

/// 顶层运行时编排接口。
///
/// 实现负责把 Observe → Understand → Decide → Validate → Execute → Measure
/// 串成一个同步 tick；调度（定时器、线程）由调用方负责。
pub trait IRuntimeOrchestrator: Send + Sync {
    fn tick(&self, targets: &ExecutionTargets) -> TickOutcome;
    fn current_mode(&self) -> RuntimeMode;
    fn metrics(&self) -> LoopMetrics;
    fn name(&self) -> &'static str;
}
