use std::sync::atomic::{AtomicU64, Ordering};

/// 推理指标。
///
/// 全部为原子计数；无锁读取。
#[derive(Default)]
pub struct InferenceMetrics {
    total_inferences: AtomicU64,
    successful: AtomicU64,
    failed: AtomicU64,
    timeouts: AtomicU64,
    last_latency_ms: AtomicU64,
    total_latency_ms: AtomicU64,
    max_latency_ms: AtomicU64,
}

impl InferenceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&self, latency_ms: u64) {
        self.total_inferences.fetch_add(1, Ordering::SeqCst);
        self.successful.fetch_add(1, Ordering::SeqCst);
        self.last_latency_ms.store(latency_ms, Ordering::SeqCst);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
        self.max_latency_ms.fetch_max(latency_ms, Ordering::SeqCst);
    }

    pub fn record_failure(&self, latency_ms: u64) {
        self.total_inferences.fetch_add(1, Ordering::SeqCst);
        self.failed.fetch_add(1, Ordering::SeqCst);
        self.last_latency_ms.store(latency_ms, Ordering::SeqCst);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
        self.max_latency_ms.fetch_max(latency_ms, Ordering::SeqCst);
    }

    pub fn record_timeout(&self, latency_ms: u64) {
        self.total_inferences.fetch_add(1, Ordering::SeqCst);
        self.timeouts.fetch_add(1, Ordering::SeqCst);
        self.last_latency_ms.store(latency_ms, Ordering::SeqCst);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
        self.max_latency_ms.fetch_max(latency_ms, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> InferenceMetricsSnapshot {
        InferenceMetricsSnapshot {
            total_inferences: self.total_inferences.load(Ordering::SeqCst),
            successful: self.successful.load(Ordering::SeqCst),
            failed: self.failed.load(Ordering::SeqCst),
            timeouts: self.timeouts.load(Ordering::SeqCst),
            last_latency_ms: self.last_latency_ms.load(Ordering::SeqCst),
            total_latency_ms: self.total_latency_ms.load(Ordering::SeqCst),
            max_latency_ms: self.max_latency_ms.load(Ordering::SeqCst),
        }
    }

    pub fn reset(&self) {
        self.total_inferences.store(0, Ordering::SeqCst);
        self.successful.store(0, Ordering::SeqCst);
        self.failed.store(0, Ordering::SeqCst);
        self.timeouts.store(0, Ordering::SeqCst);
        self.last_latency_ms.store(0, Ordering::SeqCst);
        self.total_latency_ms.store(0, Ordering::SeqCst);
        self.max_latency_ms.store(0, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferenceMetricsSnapshot {
    pub total_inferences: u64,
    pub successful: u64,
    pub failed: u64,
    pub timeouts: u64,
    pub last_latency_ms: u64,
    pub total_latency_ms: u64,
    pub max_latency_ms: u64,
}

impl InferenceMetricsSnapshot {
    pub fn avg_latency_ms(&self) -> u64 {
        if self.total_inferences == 0 {
            0
        } else {
            self.total_latency_ms / self.total_inferences
        }
    }
}
