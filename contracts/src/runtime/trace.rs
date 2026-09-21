use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// 跨模块追踪上下文。
///
/// 每个 tick 有唯一 `trace_id`；tick 内所有操作共享同一个 `trace_id`。
///
/// 约定：
/// - `trace_id` 格式为 `trace-{tick_id}-{unix_ms}`，便于日志检索。
/// - `started_at_unix_ms` 为 trace 创建时刻。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub tick_id: u64,
    pub started_at_unix_ms: u64,
}

impl TraceContext {
    pub fn new(tick_id: u64) -> Self {
        let now = now_unix_ms();
        Self {
            trace_id: format!("trace-{}-{}", tick_id, now),
            tick_id,
            started_at_unix_ms: now,
        }
    }

    /// 从当前时间计算 trace 已运行多久。
    pub fn elapsed_ms(&self) -> u64 {
        now_unix_ms().saturating_sub(self.started_at_unix_ms)
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
