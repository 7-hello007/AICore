use std::sync::mpsc;
use std::time::Duration;

/// 在独立线程里执行 `f`，带超时。
///
/// 语义：
/// - `f` 正常返回 → `Ok(Some(T))`。
/// - `f` 超时未返回 → `Ok(None)`（线程泄漏，但不阻塞调用方）。
/// - `f` panic → `Err`。
///
/// **注意**：超时的线程不会被强杀（Rust 无此能力），会继续运行
/// 直到自然结束。这是有意的取舍：优先保证调用方不被阻塞。
pub fn run_with_timeout<F, T>(f: F, timeout: Duration) -> Result<Option<T>, SandboxError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel::<T>();

    let handle = std::thread::Builder::new()
        .name("aicore-plugin-sandbox".to_string())
        .spawn(move || {
            let result = f();
            let _ = tx.send(result);
        })
        .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;

    match rx.recv_timeout(timeout) {
        Ok(v) => {
            // 线程正常结束
            let _ = handle.join();
            Ok(Some(v))
        },
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // 超时：不 join（避免阻塞），线程会泄漏直到自然结束
            tracing::warn!(
                component = "plugin_sandbox",
                event = "timeout",
                timeout_ms = timeout.as_millis() as u64,
                "plugin call timed out; thread will continue in background"
            );
            Ok(None)
        },
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // 线程 panic
            let _ = handle.join();
            Err(SandboxError::Panicked)
        },
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SandboxError {
    #[error("failed to spawn sandbox thread: {0}")]
    SpawnFailed(String),
    #[error("sandboxed function panicked")]
    Panicked,
}
