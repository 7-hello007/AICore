use std::sync::mpsc;
use std::time::Duration;

/// Run `f` in an independent line, with timeout.
///
/// Semantics:
/// - `f` returns normally to `Ok(Some(T))`.
/// - `f` did not return `Ok(None)` over time (line leak without blocking callers).
/// - `f` panic → `Err`。
///
/// **Note: Timeout threads will not be killed.
/// The worker may continue until it exits naturally. This trade-off keeps the caller from blocking.
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
            // It's over.
            let _ = handle.join();
            Ok(Some(v))
        },
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Timeout: nojoin, thread leaks until nature ends
            tracing::warn!(
                component = "plugin_sandbox",
                event = "timeout",
                timeout_ms = timeout.as_millis() as u64,
                "plugin call timed out; thread will continue in background"
            );
            Ok(None)
        },
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // Thread Panic
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
