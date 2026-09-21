use serde::{Deserialize, Serialize};

/// AdaptiveLoop 的累计指标。
///
/// 全部为单调递增计数；单次延迟为最后一轮值。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LoopMetrics {
    pub total_ticks: u64,
    pub successful_ticks: u64,
    pub failed_ticks: u64,
    pub policies_generated: u64,
    pub policies_rejected: u64,
    pub executions_succeeded: u64,
    pub executions_failed: u64,
    pub last_tick_duration_ms: u64,
    pub total_tick_duration_ms: u64,
    pub noop_ticks: u64,
    pub ai_inference_count: u64,
    pub ai_fallback_count: u64,
    pub safe_mode_entries: u64,
}
