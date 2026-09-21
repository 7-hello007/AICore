/// Telemetry 采样约束。
///
/// 用于 Adapter 决定"是否应该跳过本次采集"。
/// Adapter 不做调度；只做 advisory 检查。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SamplingConstraints {
    /// 两次 collect 之间的最小间隔，毫秒。
    pub min_interval_ms: u64,
}

impl SamplingConstraints {
    pub fn new(min_interval_ms: u64) -> Self {
        Self { min_interval_ms }
    }

    /// 给定 `since_last_ms`（距今一次 collect 过了多少毫秒），
    /// 判断是否应跳过本次采集。
    pub fn should_skip(&self, since_last_ms: u64) -> bool {
        since_last_ms < self.min_interval_ms
    }
}
