use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::errors::{AicoreError, AicoreResult};
use aicore_contracts::plugin::host::{
    IPluginHost, PluginCrashEvent, PluginHostState, PluginProcessHandle, PluginProcessSpec,
};

use super::protocol::{HostRequest, HostResponse, MAX_LINE_BYTES};

/// 子进程插件 Host。
///
/// 每个插件运行在独立进程中；Core 通过 stdin/stdout 发送 JSON 行协议。
///
/// 隔离语义：
/// - 插件崩溃 → 只影响该子进程；Host 检测到后返回 `Crashed`。
/// - 超时 → `kill()` 子进程（Rust 允许对 `Child` 强杀）。
/// - Core 自身不受影响。
pub struct SubprocessPluginHost {
    processes: Mutex<HashMap<String, ManagedChild>>,
    next_seq: AtomicU64,
    default_timeout: Duration,
    crashes: Mutex<Vec<PluginCrashEvent>>,
}

struct ManagedChild {
    child: Child,
    stdin: ChildStdin,
    stdout: Option<BufReader<ChildStdout>>,
    spec: PluginProcessSpec,
}

impl SubprocessPluginHost {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            next_seq: AtomicU64::new(1),
            default_timeout: Duration::from_secs(5),
            crashes: Mutex::new(Vec::new()),
        }
    }

    pub fn with_default_timeout(mut self, d: Duration) -> Self {
        self.default_timeout = d;
        self
    }

    /// 发送请求并等待响应（带超时）。
    pub fn call(
        &self,
        handle: &PluginProcessHandle,
        request: &HostRequest,
    ) -> AicoreResult<HostResponse> {
        let mut procs = self
            .processes
            .lock()
            .map_err(|_| AicoreError::LockPoisoned("plugin host lock poisoned".to_string()))?;
        let entry = procs.get_mut(&handle.handle_id).ok_or_else(|| {
            AicoreError::NotFound(format!("unknown handle: {}", handle.handle_id))
        })?;

        let timeout = if entry.spec.timeout_ms == 0 {
            self.default_timeout
        } else {
            Duration::from_millis(entry.spec.timeout_ms.max(100))
        };

        // 写请求（若子进程已退出，write 会返回 EPIPE/BrokenPipe）
        let json = serde_json::to_string(request)
            .map_err(|e| AicoreError::PluginFailed(format!("serialize request: {}", e)))?;
        if json.len() > MAX_LINE_BYTES {
            return Err(AicoreError::InvalidRequest(
                "plugin request too large".to_string(),
            ));
        }

        let write_result = (|| -> std::io::Result<()> {
            entry.stdin.write_all(json.as_bytes())?;
            entry.stdin.write_all(b"\n")?;
            entry.stdin.flush()?;
            Ok(())
        })();

        if let Err(e) = write_result {
            // 子进程已退出 / 管道已断 → 视为 crash
            let exit_code = entry.child.try_wait().ok().flatten().and_then(|s| s.code());
            self.record_crash(
                &entry.spec.plugin_id,
                &handle.handle_id,
                exit_code,
                &format!("write to plugin failed: {}", e),
            );
            return Err(AicoreError::PluginFailed(format!(
                "plugin process exited (write failed: {})",
                e
            )));
        }

        // 真正的超时：把阻塞 read 放到临时线程；超时后强杀子进程并等待线程释放 stdout。
        let start = Instant::now();
        let mut reader = entry
            .stdout
            .take()
            .ok_or_else(|| AicoreError::PluginFailed("plugin stdout unavailable".to_string()))?;
        let (tx, rx) = mpsc::channel::<(BufReader<ChildStdout>, AicoreResult<String>)>();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = match reader.read_line(&mut line) {
                Ok(0) => Err(AicoreError::PluginFailed(
                    "plugin process exited".to_string(),
                )),
                Ok(_) => Ok(line),
                Err(e) => Err(AicoreError::PluginFailed(format!("read failed: {}", e))),
            };
            let _ = tx.send((reader, result));
        });

        match rx.recv_timeout(timeout) {
            Ok((reader, result)) => {
                entry.stdout = Some(reader);
                let line = result?;
                serde_json::from_str::<HostResponse>(line.trim())
                    .map_err(|e| AicoreError::PluginFailed(format!("invalid response: {}", e)))
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = entry.child.kill();
                let _ = entry.child.wait();
                let _ = rx.recv_timeout(Duration::from_millis(250));
                self.record_crash(
                    &entry.spec.plugin_id,
                    &handle.handle_id,
                    None,
                    &format!(
                        "plugin call timeout after {} ms",
                        start.elapsed().as_millis()
                    ),
                );
                Err(AicoreError::Timeout(format!(
                    "plugin call timeout after {} ms",
                    start.elapsed().as_millis()
                )))
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(AicoreError::PluginFailed(
                "plugin reader thread disconnected".to_string(),
            )),
        }
    }

    fn record_crash(&self, plugin_id: &str, handle_id: &str, exit_code: Option<i32>, reason: &str) {
        if let Ok(mut crashes) = self.crashes.lock() {
            crashes.push(PluginCrashEvent {
                plugin_id: plugin_id.to_string(),
                handle_id: handle_id.to_string(),
                exit_code,
                reason: reason.to_string(),
                detected_at_unix_ms: now_unix_ms(),
            });
            if crashes.len() > 256 {
                let drop_count = crashes.len() - 256;
                crashes.drain(0..drop_count);
            }
        }
    }
}

impl Default for SubprocessPluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl IPluginHost for SubprocessPluginHost {
    fn spawn(&self, spec: &PluginProcessSpec) -> AicoreResult<PluginProcessHandle> {
        spec.validate()?;

        let mut child = Command::new(&spec.artifact_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AicoreError::PluginFailed(format!("spawn failed: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AicoreError::PluginFailed("no stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AicoreError::PluginFailed("no stdout".to_string()))?;
        if let Some(stderr) = child.stderr.take() {
            spawn_stderr_drain(spec.plugin_id.clone(), stderr);
        }
        let pid = child.id();
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let handle_id = format!("plugin-proc-{}-{}", pid, seq);

        let handle = PluginProcessHandle {
            handle_id: handle_id.clone(),
            plugin_id: spec.plugin_id.clone(),
            pid,
            started_at_unix_ms: now_unix_ms(),
        };

        let managed = ManagedChild {
            child,
            stdin,
            stdout: Some(BufReader::new(stdout)),
            spec: spec.clone(),
        };

        let mut procs = self
            .processes
            .lock()
            .map_err(|_| AicoreError::LockPoisoned("plugin host lock poisoned".to_string()))?;
        procs.insert(handle_id, managed);

        tracing::info!(
            component = "subprocess_plugin_host",
            event = "spawned",
            plugin_id = %spec.plugin_id,
            pid = pid,
            "plugin process spawned"
        );

        Ok(handle)
    }

    fn stop(&self, handle: &PluginProcessHandle) -> AicoreResult<()> {
        let mut procs = self
            .processes
            .lock()
            .map_err(|_| AicoreError::LockPoisoned("plugin host lock poisoned".to_string()))?;
        let mut entry = procs.remove(&handle.handle_id).ok_or_else(|| {
            AicoreError::NotFound(format!("unknown handle: {}", handle.handle_id))
        })?;

        // 先尝试 Shutdown 消息
        let shutdown = serde_json::to_string(&HostRequest::Shutdown).unwrap_or_default();
        let _ = entry.stdin.write_all(shutdown.as_bytes());
        let _ = entry.stdin.write_all(b"\n");
        let _ = entry.stdin.flush();

        // 给 500ms 优雅退出，否则 kill
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match entry.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = entry.child.kill();
                        let _ = entry.child.wait();
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                },
                Err(e) => return Err(AicoreError::PluginFailed(format!("wait failed: {}", e))),
            }
        }
    }

    fn state(&self, handle: &PluginProcessHandle) -> PluginHostState {
        let mut procs = match self.processes.lock() {
            Ok(p) => p,
            Err(_) => return PluginHostState::Failed,
        };
        let entry = match procs.get_mut(&handle.handle_id) {
            Some(e) => e,
            None => return PluginHostState::Stopped,
        };
        match entry.child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    PluginHostState::Stopped
                } else {
                    PluginHostState::Crashed
                }
            },
            Ok(None) => PluginHostState::Running,
            Err(_) => PluginHostState::Failed,
        }
    }

    fn recent_crashes(&self) -> Vec<PluginCrashEvent> {
        self.crashes.lock().map(|c| c.clone()).unwrap_or_default()
    }

    fn clear_crashes(&self) {
        if let Ok(mut c) = self.crashes.lock() {
            c.clear();
        }
    }

    fn host_name(&self) -> &'static str {
        "subprocess_plugin_host"
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn spawn_stderr_drain(plugin_id: String, stderr: ChildStderr) {
    std::thread::Builder::new()
        .name(format!("aicore-plugin-stderr-{}", plugin_id))
        .spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        tracing::warn!(component="subprocess_plugin_host", plugin_id=%plugin_id, event="plugin_stderr", message=%line, "plugin stderr");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::debug!(component="subprocess_plugin_host", plugin_id=%plugin_id, event="plugin_stderr_read_failed", error=%e, "plugin stderr drain stopped");
                        break;
                    }
                }
            }
        })
        .ok();
}
