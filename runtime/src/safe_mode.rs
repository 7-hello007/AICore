use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

use aicore_contracts::runtime::{config::AdaptiveLoopConfig, mode::RuntimeMode};

/// SafeMode 控制器。
///
/// 根据连续成功 / 失败数决定当前 RuntimeMode。
/// 线程安全；可在多线程环境中共享。
pub struct SafeModeController {
    config: AdaptiveLoopConfig,
    consecutive_failures: AtomicU32,
    consecutive_successes: AtomicU32,
    mode: RwLock<RuntimeMode>,
}

impl SafeModeController {
    pub fn new(config: AdaptiveLoopConfig) -> Self {
        Self {
            config,
            consecutive_failures: AtomicU32::new(0),
            consecutive_successes: AtomicU32::new(0),
            mode: RwLock::new(RuntimeMode::Normal),
        }
    }

    pub fn config(&self) -> &AdaptiveLoopConfig {
        &self.config
    }

    /// 记录一次成功，返回切换后的模式。
    pub fn record_success(&self) -> RuntimeMode {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        let successes = self.consecutive_successes.fetch_add(1, Ordering::SeqCst) + 1;

        if successes < self.config.recovery_threshold {
            return self.current_mode();
        }

        let mut mode = self.mode.write().expect("safe_mode lock poisoned");
        let previous = *mode;
        let next = match previous {
            RuntimeMode::Safe => RuntimeMode::Recovery,
            RuntimeMode::Recovery => RuntimeMode::Normal,
            RuntimeMode::Degraded => RuntimeMode::Normal,
            RuntimeMode::Normal => RuntimeMode::Normal,
        };

        if next != previous || matches!(previous, RuntimeMode::Normal) {
            self.consecutive_successes.store(0, Ordering::SeqCst);
        }
        *mode = next;
        next
    }

    /// 记录一次失败，返回切换后的模式。
    pub fn record_failure(&self) -> RuntimeMode {
        self.consecutive_successes.store(0, Ordering::SeqCst);
        let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;

        let mut mode = self.mode.write().expect("safe_mode lock poisoned");
        let next = if failures >= self.config.safe_threshold {
            RuntimeMode::Safe
        } else if failures >= self.config.degraded_threshold {
            RuntimeMode::Degraded
        } else {
            *mode
        };
        *mode = next;
        next
    }

    pub fn current_mode(&self) -> RuntimeMode {
        *self.mode.read().expect("safe_mode lock poisoned")
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    pub fn consecutive_successes(&self) -> u32 {
        self.consecutive_successes.load(Ordering::SeqCst)
    }

    /// 重置到 Normal，用于人工恢复。
    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.consecutive_successes.store(0, Ordering::SeqCst);
        if let Ok(mut mode) = self.mode.write() {
            *mode = RuntimeMode::Normal;
        }
    }
}
