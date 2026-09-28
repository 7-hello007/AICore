use std::collections::VecDeque;
use std::sync::RwLock;

use aicore_contracts::runtime::{daemon_status::TickSummary, tick::TickOutcome};

/// Circle tick history.
///
/// Keeps a recent N tick digest for CLI/ UI query.
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

    /// Default 200.
    pub fn with_default_capacity() -> Self {
        Self::new(200)
    }

    /// Adds a tick to evicts the oldest when exceeding capacity.
    pub fn append(&self, outcome: &TickOutcome) {
        let summary = TickSummary::from_outcome(outcome);
        if let Ok(mut buf) = self.inner.write() {
            while buf.len() >= self.max {
                buf.pop_front();
            }
            buf.push_back(summary);
        }
    }

    /// Returns the recent `limit` entry, sorting from old to new.
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
