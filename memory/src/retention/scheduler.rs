use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aicore_contracts::{
    errors::ContractResult,
    memory::retention::{RetentionPolicy, RetentionReport},
    traits::memory_store::IMemoryStore,
};

/// 保留调度器。
///
/// 语义：
/// - 无副作用地判断是否该运行（`should_run`）。
/// - 每次运行产出 `RetentionReport`（同时写入审计缓冲）。
/// - `retention_days` 从 `UserProfile` 读取，作为唯一权威。
/// - cutoff 由传入的 `now` 计算，保证 `run_once_at(now)` 的时间可控。
/// - 失败不会 panic，报告 `RetentionReport::failure`。
///
/// 时间语义：
/// - 所有运行时间与记录时间统一使用 Unix milliseconds (`unix_ms`)。
/// - `RetentionPolicy.check_interval_secs` 仍保留“秒”作为配置单位，
///   在调度器内部转换为毫秒进行比较。
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

    /// 判断是否应在本时间点运行。
    ///
    /// `now` 使用 Unix milliseconds。
    ///
    /// - `policy.enabled = false` → 永远 false。
    /// - 从未运行过 → true（首次立即运行）。
    /// - 距离上次运行 ≥ `check_interval_secs` → true。
    pub fn should_run(&self, now: u64) -> bool {
        if !self.policy.enabled {
            return false;
        }

        let last = self.last_run_unix_ms.load(Ordering::SeqCst);

        if last == 0 {
            return true;
        }

        // 配置仍然以秒保存，因此比较前转换成毫秒。
        let check_interval_ms = self.policy.check_interval_secs.saturating_mul(1_000);

        now.saturating_sub(last) >= check_interval_ms
    }

    /// 使用当前时间运行（供生产环境）。
    pub fn run_once(&self) -> ContractResult<RetentionReport> {
        self.run_once_at(now_unix_ms())
    }

    /// 在指定时间点运行（供测试）。
    ///
    /// `now` 使用 Unix milliseconds。
    pub fn run_once_at(&self, now: u64) -> ContractResult<RetentionReport> {
        let report = self.evaluate(now);

        if let Ok(mut audit) = self.audit.lock() {
            while audit.len() >= self.policy.audit_capacity {
                audit.pop_front();
            }

            audit.push_back(report.clone());
        }

        // 无论成功、失败、跳过，都更新 last_run，
        // 避免调度器在连续调用时发生紧密重试。
        self.last_run_unix_ms.store(now, Ordering::SeqCst);

        Ok(report)
    }

    fn evaluate(&self, now: u64) -> RetentionReport {
        // 1. 读取 profile
        let profile = match self.store.load_profile() {
            Ok(p) => p,
            Err(e) => {
                return RetentionReport::failure(now, 0, format!("load_profile: {}", e));
            },
        };

        // 2. retention_days = 0 → 表示无限保留，跳过
        if profile.retention_days == 0 {
            return RetentionReport::skipped(now, 0, "retention_days = 0 (unlimited)");
        }

        // 3. 根据传入的 now 计算 cutoff。
        //
        // 不调用 store.apply_retention(days)，因为它可能使用
        // 实际系统时间，而 run_once_at(now) 要求整个过程使用
        // 调用方传入的确定性时间。
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

    /// 返回审计日志（从旧到新）。
    pub fn audit_log(&self) -> Vec<RetentionReport> {
        self.audit
            .lock()
            .map(|a| a.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 返回最近一次运行时间。
    ///
    /// 返回值使用 Unix milliseconds。
    /// 0 表示从未运行。
    pub fn last_run_unix_ms(&self) -> u64 {
        self.last_run_unix_ms.load(Ordering::SeqCst)
    }

    /// 清空审计日志。
    pub fn clear_audit(&self) {
        if let Ok(mut audit) = self.audit.lock() {
            audit.clear();
        }
    }
}

/// 当前 Unix 时间，单位：milliseconds。
fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
