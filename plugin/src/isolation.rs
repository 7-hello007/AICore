use std::sync::atomic::{AtomicU32, Ordering};

use aicore_contracts::plugin::state::PluginState;

/// 单个插件的错误边界。
///
/// 语义：
/// - 记录连续失败。
/// - 超过阈值自动置 Failed。
/// - 只影响该插件，不影响其他插件或 Core。
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

    /// 记录一次失败。返回是否应隔离（转为 Failed）。
    pub fn record_failure(&self) -> bool {
        let n = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        n >= self.failure_threshold
    }

    /// 记录一次成功。重置失败计数。
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
    }

    /// 根据当前状态，推荐下一步 PluginState。
    pub fn recommended_state(&self, current: PluginState) -> PluginState {
        if current == PluginState::Active && self.failure_count() >= self.failure_threshold {
            PluginState::Failed
        } else {
            current
        }
    }
}
