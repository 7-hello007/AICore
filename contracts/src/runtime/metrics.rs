use serde::{Deserialize, Serialize};

/// Cumulative indicator for AdaptiveLoop.
///
/// ; single delay is the last round.
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
    /// Measured only by the split Model Mode driver, in microseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fast_control_us: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_slow_prepare_us: Option<u64>,
}
