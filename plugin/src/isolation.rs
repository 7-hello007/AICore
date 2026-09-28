use std::sync::atomic::{AtomicU32, Ordering};

use aicore_contracts::plugin::state::PluginState;

/// The wrong border for a single plugin.
///
/// Semantics:
/// - Record failure.
/// - Auto-set Failed beyond the threshold.
/// - affect only the plugin and not other plugins or Core.
#[derive(Debug)]
pub struct PluginIsolation {
    name: String,
    failure_threshold: u32,
    failure_count: AtomicU32,
}

impl PluginIsolation {
    pub fn new(name: impl Into<String>, failure_threshold: u32) -> Self {
        Self {
            name: name.into(),
            failure_threshold: failure_threshold.max(1),
            failure_count: AtomicU32::new(0),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn failure_threshold(&self) -> u32 {
        self.failure_threshold
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Record a failure. Should return be isolated (to Failed).
    pub fn record_failure(&self) -> bool {
        let n = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        n >= self.failure_threshold
    }

    /// Record a success. Reset the failure count.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
    }

    /// Next PluginState is recommended according to current status.
    pub fn recommended_state(&self, current: PluginState) -> PluginState {
        if current == PluginState::Active && self.failure_count() >= self.failure_threshold {
            PluginState::Failed
        } else {
            current
        }
    }
}
