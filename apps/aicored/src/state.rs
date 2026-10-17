use std::path::{Path, PathBuf};

use aicore_contracts::runtime::daemon_status::DaemonStatus;

/// One coalesced pending snapshot; disk IO never holds the caller's state lock.
pub struct StatusWriter {
    latest: std::sync::Arc<std::sync::Mutex<Option<DaemonStatus>>>,
    wake: std::sync::mpsc::SyncSender<()>,
    done: std::sync::mpsc::Receiver<std::io::Result<()>>,
}

impl StatusWriter {
    pub fn start(path: PathBuf) -> std::io::Result<Self> {
        Self::start_with(move |status| write_status_atomic(&path, status))
    }

    fn start_with(
        write: impl Fn(&DaemonStatus) -> std::io::Result<()> + Send + 'static,
    ) -> std::io::Result<Self> {
        let latest = std::sync::Arc::new(std::sync::Mutex::new(None));
        let worker_latest = latest.clone();
        let (wake, signals) = std::sync::mpsc::sync_channel(1);
        let (finished, done) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new().name("aicore-status-writer".into()).spawn(move || {
            let mut result = Ok(());
            while signals.recv().is_ok() {
                let snapshot = match worker_latest.lock() {
                    Ok(mut pending) => pending.take(),
                    Err(_) => { result = Err(std::io::Error::new(std::io::ErrorKind::Other, "status writer lock poisoned")); break; }
                };
                if let Some(status) = snapshot {
                    result = write(&status);
                    if let Err(error) = &result { tracing::warn!(component="aicored", event="state_write_failed", error=%error); }
                }
            }
            let _ = finished.send(result);
        })?;
        Ok(Self { latest, wake, done })
    }

    pub fn submit(&self, status: DaemonStatus) -> bool {
        let Ok(mut pending) = self.latest.lock() else {
            return false;
        };
        *pending = Some(status);
        drop(pending);
        !matches!(
            self.wake.try_send(()),
            Err(std::sync::mpsc::TrySendError::Disconnected(_))
        )
    }

    pub fn finish(self, timeout: std::time::Duration) -> std::io::Result<()> {
        drop(self.wake);
        self.done
            .recv_timeout(timeout)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::TimedOut, error))?
    }
}

/// Default status file path.
pub fn default_state_path() -> PathBuf {
    std::env::temp_dir().join("aicore").join("status.json")
}

/// Atomic writing status file.
///
/// Write `.tmp` and rename to avoid reading semi-document.
pub fn write_status_atomic(path: &Path, status: &DaemonStatus) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(status)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn p1_slow_persistence_coalesces_without_blocking_and_flushes_last_snapshot() {
        let (entered, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let output = recorded.clone();
        let writer = StatusWriter::start_with(move |status| {
            if status.tick_count == 1 {
                entered.send(()).unwrap();
                resume.recv().unwrap();
            }
            output.lock().unwrap().push(status.tick_count);
            Ok(())
        })
        .unwrap();
        let mut status = DaemonStatus::new(1);
        status.tick_count = 1;
        assert!(writer.submit(status.clone()));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        let start = Instant::now();
        for tick in 2..=100 {
            status.tick_count = tick;
            assert!(writer.submit(status.clone()));
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
        writer.finish(Duration::from_secs(1)).unwrap();
        assert_eq!(*recorded.lock().unwrap(), vec![1, 100]);
    }

    #[test]
    fn p1_persistence_failure_is_reported_at_shutdown() {
        let writer = StatusWriter::start_with(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "write fault",
            ))
        })
        .unwrap();
        assert!(writer.submit(DaemonStatus::new(1)));
        assert_eq!(
            writer.finish(Duration::from_secs(1)).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn p1_blocked_persistence_shutdown_obeys_its_deadline() {
        let (entered, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let writer = StatusWriter::start_with(move |_| {
            entered.send(()).unwrap();
            let _ = resume.recv_timeout(Duration::from_secs(1));
            let _ = finished.send(());
            Ok(())
        })
        .unwrap();
        assert!(writer.submit(DaemonStatus::new(1)));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        let start = Instant::now();
        let result = writer.finish(Duration::from_millis(30));
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_millis(100));
    }
}
