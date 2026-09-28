use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aicore_contracts::runtime::metrics::LoopMetrics;

/// Indicator collector.
///
/// All atoms; one-time take-out when read.
#[derive(Default)]
pub struct MetricsCollector {
    total_ticks: AtomicU64,
    successful_ticks: AtomicU64,
    failed_ticks: AtomicU64,
    policies_generated: AtomicU64,
    policies_rejected: AtomicU64,
    executions_succeeded: AtomicU64,
    executions_failed: AtomicU64,
    last_tick_duration_ms: AtomicU64,
    total_tick_duration_ms: AtomicU64,
    noop_ticks: AtomicU64,
    ai_inference_count: AtomicU64,
    ai_fallback_count: AtomicU64,
    safe_mode_entries: AtomicU64,
    last_fast_control_us: AtomicU64,
    last_slow_prepare_us: AtomicU64,
    has_fast_control: AtomicBool,
    has_slow_prepare: AtomicBool,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_tick(&self, success: bool, duration_ms: u64) {
        self.total_ticks.fetch_add(1, Ordering::SeqCst);
        if success {
            self.successful_ticks.fetch_add(1, Ordering::SeqCst);
        } else {
            self.failed_ticks.fetch_add(1, Ordering::SeqCst);
        }
        self.last_tick_duration_ms
            .store(duration_ms, Ordering::SeqCst);
        self.total_tick_duration_ms
            .fetch_add(duration_ms, Ordering::SeqCst);
    }

    pub fn record_noop_tick(&self) {
        self.noop_ticks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_neutral_tick(&self, duration_ms: u64) {
        self.total_ticks.fetch_add(1, Ordering::SeqCst);
        self.last_tick_duration_ms
            .store(duration_ms, Ordering::SeqCst);
        self.total_tick_duration_ms
            .fetch_add(duration_ms, Ordering::SeqCst);
        self.record_noop_tick();
    }

    pub fn record_fast_control(&self, duration: std::time::Duration) {
        self.last_fast_control_us.store(
            duration.as_micros().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        self.has_fast_control.store(true, Ordering::Release);
    }

    pub fn record_slow_prepare(&self, duration: std::time::Duration) {
        self.last_slow_prepare_us.store(
            duration.as_micros().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        self.has_slow_prepare.store(true, Ordering::Release);
    }

    pub fn record_ai_inference(&self) {
        self.ai_inference_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_ai_fallback(&self) {
        self.ai_fallback_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_safe_mode_entry(&self) {
        self.safe_mode_entries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_policy_generated(&self) {
        self.policies_generated.fetch_add(1, Ordering::SeqCst);
    }

    pub fn record_policy_rejected(&self) {
        self.policies_rejected.fetch_add(1, Ordering::SeqCst);
    }

    pub fn record_execution(&self, success: bool) {
        if success {
            self.executions_succeeded.fetch_add(1, Ordering::SeqCst);
        } else {
            self.executions_failed.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn snapshot(&self) -> LoopMetrics {
        LoopMetrics {
            total_ticks: self.total_ticks.load(Ordering::Relaxed),
            successful_ticks: self.successful_ticks.load(Ordering::Relaxed),
            failed_ticks: self.failed_ticks.load(Ordering::Relaxed),
            policies_generated: self.policies_generated.load(Ordering::Relaxed),
            policies_rejected: self.policies_rejected.load(Ordering::Relaxed),
            executions_succeeded: self.executions_succeeded.load(Ordering::Relaxed),
            executions_failed: self.executions_failed.load(Ordering::Relaxed),
            last_tick_duration_ms: self.last_tick_duration_ms.load(Ordering::Relaxed),
            total_tick_duration_ms: self.total_tick_duration_ms.load(Ordering::Relaxed),
            noop_ticks: self.noop_ticks.load(Ordering::Relaxed),
            ai_inference_count: self.ai_inference_count.load(Ordering::Relaxed),
            ai_fallback_count: self.ai_fallback_count.load(Ordering::Relaxed),
            safe_mode_entries: self.safe_mode_entries.load(Ordering::Relaxed),
            last_fast_control_us: self
                .has_fast_control
                .load(Ordering::Acquire)
                .then(|| self.last_fast_control_us.load(Ordering::Relaxed)),
            last_slow_prepare_us: self
                .has_slow_prepare
                .load(Ordering::Acquire)
                .then(|| self.last_slow_prepare_us.load(Ordering::Relaxed)),
        }
    }
}
