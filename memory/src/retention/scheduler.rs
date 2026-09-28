use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aicore_contracts::{
    errors::ContractResult,
    memory::retention::{RetentionPolicy, RetentionReport},
    traits::memory_store::IMemoryStore,
};

/// Retention scheduler.
///
/// Semantics:
/// - `should_run` decides without side effects.
/// - Each run returns a `RetentionReport` and writes it to the audit buffer.
/// - `retention_days` read from `UserProfile` as sole authority.
/// - The supplied `now` determines the cutoff, making `run_once_at(now)` deterministic.
/// - Failures are reported through `RetentionReport::failure` without panicking.
///
/// Time semantics:
/// - Run and record timestamps use Unix milliseconds (`unix_ms`).
/// - `RetentionPolicy.check_interval_secs` remains configured in seconds and
///   is converted to milliseconds before comparison.
pub struct RetentionScheduler {
    store: Arc<dyn IMemoryStore>,
    policy: RetentionPolicy,
    audit: Mutex<VecDeque<RetentionReport>>,
    last_run_unix_ms: AtomicU64,
}

impl RetentionScheduler {
    pub fn new(store: Arc<dyn IMemoryStore>, policy: RetentionPolicy) -> ContractResult<Self> {
        policy.validate()?;

        let capacity = policy.audit_capacity;

        Ok(Self {
            store,
            policy,
            audit: Mutex::new(VecDeque::with_capacity(capacity)),
            last_run_unix_ms: AtomicU64::new(0),
        })
    }

    pub fn policy(&self) -> &RetentionPolicy {
        &self.policy
    }

    /// Determines whether it should run at this time point.
    ///
    /// `now` uses Unix milliseconds.
    ///
    /// - `policy.enabled = false` → always false.
    /// - Never run → true, so the first run is immediate.
    /// - At least `check_interval_secs` since the last run → true.
    pub fn should_run(&self, now: u64) -> bool {
        if !self.policy.enabled {
            return false;
        }

        let last = self.last_run_unix_ms.load(Ordering::SeqCst);

        if last == 0 {
            return true;
        }

        // The configuration is still saved in seconds and is therefore converted to milliseconds before comparison.
        let check_interval_ms = self.policy.check_interval_secs.saturating_mul(1_000);

        now.saturating_sub(last) >= check_interval_ms
    }

    /// Run using current time (for the production environment).
    pub fn run_once(&self) -> ContractResult<RetentionReport> {
        self.run_once_at(now_unix_ms())
    }

    /// Run at the specified time point (for testing).
    ///
    /// `now` uses Unix milliseconds.
    pub fn run_once_at(&self, now: u64) -> ContractResult<RetentionReport> {
        let report = self.evaluate(now);

        if let Ok(mut audit) = self.audit.lock() {
            while audit.len() >= self.policy.audit_capacity {
                audit.pop_front();
            }

            audit.push_back(report.clone());
        }

        // Update last_run after success, failure, or skip.
        // This avoids tight retries when the scheduler is called repeatedly.
        self.last_run_unix_ms.store(now, Ordering::SeqCst);

        Ok(report)
    }

    fn evaluate(&self, now: u64) -> RetentionReport {
        // 1. Read the profile.
        let profile = match self.store.load_profile() {
            Ok(p) => p,
            Err(e) => {
                return RetentionReport::failure(now, 0, format!("load_profile: {}", e));
            },
        };

        // 2. retention_days = 0 means unlimited retention; skip cleanup.
        if profile.retention_days == 0 {
            return RetentionReport::skipped(now, 0, "retention_days = 0 (unlimited)");
        }

        // 3. Calculate the cutoff from the supplied now.
        //
        // Do not call store.apply_retention(days): it may use wall-clock time,
        // while run_once_at(now) requires deterministic time throughout.
        // Use the caller-supplied timestamp for every step instead.
        const DAY_MS: u64 = 86_400_000;

        let retention_ms = (profile.retention_days as u64).saturating_mul(DAY_MS);

        let cutoff_unix_ms = now.saturating_sub(retention_ms);

        match self.store.delete_records_before(cutoff_unix_ms) {
            Ok(removed) => RetentionReport::success(now, profile.retention_days, removed),

            Err(e) => RetentionReport::failure(
                now,
                profile.retention_days,
                format!("delete_records_before: {}", e),
            ),
        }
    }

    /// Returns the audit log (from old to new).
    pub fn audit_log(&self) -> Vec<RetentionReport> {
        self.audit
            .lock()
            .map(|a| a.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the last run time.
    ///
    /// Returns values with Unix milliseconds.
    /// 0 indicates never running.
    pub fn last_run_unix_ms(&self) -> u64 {
        self.last_run_unix_ms.load(Ordering::SeqCst)
    }

    /// Clear the audit log.
    pub fn clear_audit(&self) {
        if let Ok(mut audit) = self.audit.lock() {
            audit.clear();
        }
    }
}

/// Current Unix time, in milliseconds.
fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
