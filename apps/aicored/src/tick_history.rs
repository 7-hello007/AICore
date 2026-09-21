use std::collections::VecDeque;
use std::sync::RwLock;

use aicore_contracts::runtime::{daemon_status::TickSummary, tick::TickOutcome};

/// 环形 tick 历史。
///
/// 保留最近 N 条 tick 摘要，供 CLI / UI 查询。
pub struct TickHistory {
    max: usize,
    inner: RwLock<VecDeque<TickSummary>>,
}

impl TickHistory {
    pub fn new(max: usize) -> Self {
        Self {
            max: max.max(1),
            inner: RwLock::new(VecDeque::with_capacity(max)),
        }
    }

    /// 默认 200 条。
    pub fn with_default_capacity() -> Self {
        Self::new(200)
    }

    /// 追加一条 tick，超出容量时挤出最旧的。
    pub fn append(&self, outcome: &TickOutcome) {
        let summary = TickSummary::from_outcome(outcome);
        if let Ok(mut buf) = self.inner.write() {
            while buf.len() >= self.max {
                buf.pop_front();
            }
            buf.push_back(summary);
        }
    }

    /// 返回最近 `limit` 条，按从旧到新排序。
    pub fn snapshot(&self, limit: usize) -> Vec<TickSummary> {
        let buf = match self.inner.read() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let n = limit.min(buf.len());
        let start = buf.len().saturating_sub(n);
        buf.iter().skip(start).cloned().collect()
    }
}

impl Default for TickHistory {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}
