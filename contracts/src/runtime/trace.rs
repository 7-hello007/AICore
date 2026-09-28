use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Track context across modules.
///
/// Each tick has the only `trace_id`; all operations within tick share the same `trace_id`.
///
/// Have agreed:
/// - `trace_id` is formatted as `trace-{tick_id}-{unix_ms}`, which facilitates log search.
/// - `started_at_unix_ms` creates a moment for a trace.
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

    /// Calculates how long the track has been running from the current time.
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
