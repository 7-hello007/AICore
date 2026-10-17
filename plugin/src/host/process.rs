use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, TryLockError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::errors::{AicoreError, AicoreResult};
use aicore_contracts::plugin::host::{
    IPluginHost, PluginCrashEvent, PluginHostState, PluginProcessHandle, PluginProcessSpec,
};

use super::protocol::{HostRequest, HostResponse, MAX_LINE_BYTES};

/// Subprocess plugin Host.
///
/// Each plugin runs in an independent process; Core sends JSON row protocols via stdin/stdout.
///
/// Separator syntax:
/// - Plugin crashes affect only the subprocess; Host detects and returns `Crashed`.
/// - Timeout `kill()` subprocess (Rust allows the killing of `Child`).
/// - Core does not affect itself.
pub struct SubprocessPluginHost {
    processes: Mutex<HashMap<String, Arc<ManagedChild>>>,
    next_seq: AtomicU64,
    default_timeout: Duration,
    crashes: Mutex<Vec<PluginCrashEvent>>,
    line_limit: usize,
}

struct ManagedChild {
    isolation: crate::isolation::ProcessIsolation,
    child: Mutex<Child>,
    operation: Mutex<()>,
    requests: mpsc::Sender<IoRequest>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    cancellation: PipeCancellation,
    terminated: Arc<AtomicBool>,
    spec: PluginProcessSpec,
    exit_observed: AtomicBool,
}

struct IoRequest {
    json: String,
    reply: mpsc::Sender<AicoreResult<String>>,
}

impl ManagedChild {
    // The child lock never protects pipe IO. Killing the process can therefore
    // interrupt an active request without waiting for its operation lock.
    fn terminate(&self) {
        // Serialize cleanup so a racing stop/call waits for the same workers.
        let mut workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
        self.terminated.store(true, Ordering::Release);
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        self.isolation.terminate();
        let _ = child.kill();
        let _ = child.wait();
        drop(child);
        self.cancellation.cancel();
        for worker in workers.drain(..) {
            // Windows synchronous pipe IO also needs cancellation if another
            // process inherited the pipe. Unix pipes are nonblocking below.
            #[cfg(windows)]
            while !worker.is_finished() {
                self.cancellation.cancel();
                cancel_pipe_io(&worker);
                std::thread::sleep(Duration::from_millis(1));
            }
            let _ = worker.join();
        }
        self.cancellation.close();
    }
}

impl SubprocessPluginHost {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            next_seq: AtomicU64::new(1),
            default_timeout: Duration::from_secs(5),
            crashes: Mutex::new(Vec::new()),
            line_limit: MAX_LINE_BYTES,
        }
    }

    pub fn with_default_timeout(mut self, d: Duration) -> Self {
        self.default_timeout = d;
        self
    }

    /// IPC brokers use the existing IPC frame ceiling; plugin JSON keeps its
    /// original 64 KiB ceiling. Never accept an unbounded line allocation.
    pub fn with_line_limit(mut self, bytes: usize) -> Self {
        self.line_limit = bytes.clamp(1, 1024 * 1024);
        self
    }

    /// Sends a request and waits for a response (with timeout).
    pub fn call(
        &self,
        handle: &PluginProcessHandle,
        request: &HostRequest,
    ) -> AicoreResult<HostResponse> {
        let start = Instant::now();
        let json = serde_json::to_string(request)
            .map_err(|e| AicoreError::PluginFailed(format!("serialize request: {}", e)))?;
        let line = self.call_json_started(handle, &json, start)?;
        serde_json::from_str::<HostResponse>(line.trim())
            .map_err(|e| AicoreError::PluginFailed(format!("invalid response: {}", e)))
    }

    /// Bounded JSON-line exchange for a managed subprocess protocol. The
    /// caller validates its typed response; raw newlines are never accepted.
    pub fn call_json(&self, handle: &PluginProcessHandle, json: &str) -> AicoreResult<String> {
        self.call_json_started(handle, json, Instant::now())
    }

    fn call_json_started(
        &self,
        handle: &PluginProcessHandle,
        json: &str,
        start: Instant,
    ) -> AicoreResult<String> {
        let entry = self.lookup(handle)?;
        let timeout = if entry.spec.timeout_ms == 0 {
            self.default_timeout
        } else {
            Duration::from_millis(entry.spec.timeout_ms.max(100))
        };
        if json.contains(['\n', '\r']) {
            return Err(AicoreError::InvalidRequest(
                "JSON must be a single line".into(),
            ));
        }
        if json.len() > self.line_limit {
            return Err(AicoreError::InvalidRequest(
                "plugin request too large".to_string(),
            ));
        }

        let deadline = start + timeout;
        let operation = match lock_until(&entry.operation, deadline) {
            Ok(operation) => operation,
            Err(error) => {
                if matches!(error, AicoreError::Timeout(_)) {
                    self.fail_process(&entry, handle, &error);
                }
                return Err(error);
            },
        };
        if entry.terminated.load(Ordering::Acquire) {
            return Err(AicoreError::PluginFailed(
                "plugin process exited".to_string(),
            ));
        }
        let result = exchange(&entry, json.to_owned(), deadline);
        // Keep the operation lock until cleanup completes: no caller can reuse
        // a partially written request or consume the preceding response.
        let line = match result {
            Ok(line) => line,
            Err(error) => {
                self.fail_process(&entry, handle, &error);
                return Err(error);
            },
        };
        drop(operation);
        Ok(line)
    }

    /// Force cancellation, reap the whole owned process tree and join pipe
    /// workers. Unlike stop(), this does not attempt graceful protocol IO.
    pub fn abort(&self, handle: &PluginProcessHandle) {
        let entry = self
            .processes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle.handle_id);
        if let Some(entry) = entry {
            entry.terminate();
        }
    }

    fn lookup(&self, handle: &PluginProcessHandle) -> AicoreResult<Arc<ManagedChild>> {
        self.processes
            .lock()
            .map_err(|_| AicoreError::LockPoisoned("plugin host lock poisoned".to_string()))?
            .get(&handle.handle_id)
            .cloned()
            .ok_or_else(|| AicoreError::NotFound(format!("unknown handle: {}", handle.handle_id)))
    }

    fn fail_process(
        &self,
        entry: &ManagedChild,
        handle: &PluginProcessHandle,
        error: &AicoreError,
    ) {
        entry.terminate();
        if !entry.exit_observed.swap(true, Ordering::AcqRel) {
            self.record_crash(
                &entry.spec.plugin_id,
                &handle.handle_id,
                None,
                &error.to_string(),
            );
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

impl Drop for SubprocessPluginHost {
    fn drop(&mut self) {
        let processes = match self.processes.get_mut() {
            Ok(processes) => std::mem::take(processes),
            Err(poisoned) => std::mem::take(poisoned.into_inner()),
        };
        for (_, managed) in processes {
            managed.terminate();
        }
    }
}

impl IPluginHost for SubprocessPluginHost {
    fn spawn(&self, spec: &PluginProcessSpec) -> AicoreResult<PluginProcessHandle> {
        self.spawn_with_args(spec, &[])
    }

    fn stop(&self, handle: &PluginProcessHandle) -> AicoreResult<()> {
        self.stop_process(handle)
    }

    fn state(&self, handle: &PluginProcessHandle) -> PluginHostState {
        self.process_state(handle)
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

impl SubprocessPluginHost {
    pub fn spawn_with_args(
        &self,
        spec: &PluginProcessSpec,
        args: &[String],
    ) -> AicoreResult<PluginProcessHandle> {
        spec.validate()?;
        let mut command = Command::new(&spec.artifact_path);
        command.args(args);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (mut child, isolation) =
            crate::isolation::ProcessIsolation::spawn(&mut command, spec.memory_limit_mb)
                .map_err(|e| AicoreError::PluginFailed(format!("spawn failed: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AicoreError::PluginFailed("no stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AicoreError::PluginFailed("no stdout".to_string()))?;
        let stderr = child.stderr.take();
        let pid = child.id();
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let handle_id = format!("plugin-proc-{}-{}", pid, seq);

        let handle = PluginProcessHandle {
            handle_id: handle_id.clone(),
            plugin_id: spec.plugin_id.clone(),
            pid,
            started_at_unix_ms: now_unix_ms(),
        };

        let terminated = Arc::new(AtomicBool::new(false));
        let (requests, rx) = mpsc::channel();
        let workers = (|| -> io::Result<(Vec<JoinHandle<()>>, PipeCancellation)> {
            configure_pipe(&stdin)?;
            configure_pipe(&stdout)?;
            if let Some(stderr) = &stderr {
                configure_pipe(stderr)?;
            }
            let cancellation = PipeCancellation::new(&stdin, &stdout, stderr.as_ref())?;
            let io_worker =
                spawn_io_worker(stdin, stdout, rx, terminated.clone(), self.line_limit)?;
            let mut workers = vec![io_worker];
            if let Some(stderr) = stderr {
                match spawn_stderr_drain(spec.plugin_id.clone(), stderr, terminated.clone()) {
                    Ok(worker) => workers.push(worker),
                    Err(error) => {
                        terminated.store(true, Ordering::Release);
                        isolation.terminate();
                        let _ = child.kill();
                        let _ = child.wait();
                        for worker in workers {
                            let _ = worker.join();
                        }
                        return Err(error);
                    },
                }
            }
            Ok((workers, cancellation))
        })();
        let (workers, cancellation) = match workers {
            Ok(workers) => workers,
            Err(error) => {
                isolation.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(AicoreError::PluginFailed(format!(
                    "plugin IO setup failed: {}",
                    error
                )));
            },
        };
        let managed = Arc::new(ManagedChild {
            isolation,
            child: Mutex::new(child),
            operation: Mutex::new(()),
            requests,
            workers: Mutex::new(workers),
            cancellation,
            terminated,
            spec: spec.clone(),
            exit_observed: AtomicBool::new(false),
        });
        match self.processes.lock() {
            Ok(mut procs) => {
                procs.insert(handle_id, managed);
            },
            Err(_) => {
                managed.terminate();
                return Err(AicoreError::LockPoisoned(
                    "plugin host lock poisoned".to_string(),
                ));
            },
        }

        tracing::info!(
            component = "subprocess_plugin_host",
            event = "spawned",
            plugin_id = %spec.plugin_id,
            pid = pid,
            "plugin process spawned"
        );

        Ok(handle)
    }

    fn stop_process(&self, handle: &PluginProcessHandle) -> AicoreResult<()> {
        let deadline = Instant::now() + Duration::from_millis(500);
        let entry = self
            .processes
            .lock()
            .map_err(|_| AicoreError::LockPoisoned("plugin host lock poisoned".to_string()))?
            .remove(&handle.handle_id)
            .ok_or_else(|| {
                AicoreError::NotFound(format!("unknown handle: {}", handle.handle_id))
            })?;

        // Graceful shutdown has the same 500ms budget as the queue wait and
        // shutdown write. A blocked active call cannot prevent forced cleanup.
        if let Ok(_operation) = lock_until(&entry.operation, deadline) {
            if !entry.terminated.load(Ordering::Acquire) {
                let shutdown = serde_json::to_string(&HostRequest::Shutdown).unwrap_or_default();
                let _ = exchange(&entry, shutdown, deadline);
                while Instant::now() < deadline {
                    let mut child = entry.child.lock().map_err(|_| {
                        AicoreError::LockPoisoned("plugin child lock poisoned".to_string())
                    })?;
                    let status = entry.isolation.try_wait(&mut child);
                    drop(child);
                    match status {
                        Ok(Some(_)) => break,
                        Ok(None) => std::thread::sleep(Duration::from_millis(1)),
                        Err(error) => {
                            entry.terminate();
                            return Err(AicoreError::PluginFailed(format!(
                                "wait failed: {}",
                                error
                            )));
                        },
                    }
                }
            }
        }
        entry.terminate();
        Ok(())
    }

    fn process_state(&self, handle: &PluginProcessHandle) -> PluginHostState {
        let entry = match self.lookup(handle) {
            Ok(entry) => entry,
            Err(AicoreError::NotFound(_)) => return PluginHostState::Stopped,
            Err(_) => return PluginHostState::Failed,
        };
        let mut child = match entry.child.lock() {
            Ok(child) => child,
            Err(_) => return PluginHostState::Failed,
        };
        let status = entry.isolation.try_wait(&mut child);
        drop(child);
        match status {
            Ok(Some(status)) => {
                entry.terminate();
                if status.success() {
                    PluginHostState::Stopped
                } else {
                    if !entry.exit_observed.swap(true, Ordering::AcqRel) {
                        self.record_crash(
                            &entry.spec.plugin_id,
                            &handle.handle_id,
                            status.code(),
                            "plugin process exited unsuccessfully",
                        );
                    }
                    PluginHostState::Crashed
                }
            },
            Ok(None) => PluginHostState::Running,
            Err(_) => PluginHostState::Failed,
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(all(test, target_os = "linux"))]
mod resource_tests {
    use super::*;
    #[test]
    fn exited_leader_releases_group_before_pid_can_be_reused() {
        let host = SubprocessPluginHost::new();
        let handle = host
            .spawn(&PluginProcessSpec {
                plugin_id: "natural-exit".into(),
                artifact_path: "/bin/true".into(),
                timeout_ms: 1000,
                memory_limit_mb: 16,
            })
            .unwrap();
        let entry = host.lookup(&handle).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while host.state(&handle) == PluginHostState::Running {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(host.state(&handle), PluginHostState::Stopped);
        assert!(
            !entry.isolation.group_reserved(),
            "reaped leader left a stale process-group identity"
        );
        host.stop(&handle).unwrap();
    }
}

fn lock_until(lock: &Mutex<()>, deadline: Instant) -> AicoreResult<MutexGuard<'_, ()>> {
    loop {
        if Instant::now() >= deadline {
            return Err(AicoreError::Timeout(
                "plugin call timeout waiting for IO".to_string(),
            ));
        }
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(AicoreError::LockPoisoned(
                    "plugin IO lock poisoned".to_string(),
                ));
            },
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}

fn exchange(entry: &ManagedChild, json: String, deadline: Instant) -> AicoreResult<String> {
    if Instant::now() >= deadline {
        return Err(AicoreError::Timeout("plugin call timeout".to_string()));
    }
    let (reply, response) = mpsc::channel();
    // The operation guard permits at most one pending exchange per process.
    entry
        .requests
        .send(IoRequest { json, reply })
        .map_err(|_| {
            AicoreError::PluginFailed("plugin process exited (IO worker disconnected)".to_string())
        })?;
    response
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => {
                AicoreError::Timeout("plugin call timeout".to_string())
            },
            mpsc::RecvTimeoutError::Disconnected => {
                AicoreError::PluginFailed("plugin IO worker disconnected".to_string())
            },
        })?
}

fn spawn_io_worker(
    stdin: ChildStdin,
    stdout: ChildStdout,
    requests: mpsc::Receiver<IoRequest>,
    terminated: Arc<AtomicBool>,
    line_limit: usize,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("aicore-plugin-io".to_string())
        .spawn(move || {
            let mut stdin = Interruptible {
                stream: stdin,
                terminated: terminated.clone(),
            };
            let mut stdout = BufReader::new(Interruptible {
                stream: stdout,
                terminated: terminated.clone(),
            });
            while !terminated.load(Ordering::Acquire) {
                let request = match requests.recv_timeout(Duration::from_millis(5)) {
                    Ok(request) => request,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let result = (|| {
                    stdin
                        .write_all(request.json.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush())
                        .map_err(|error| {
                            AicoreError::PluginFailed(format!(
                                "plugin process exited (write failed: {})",
                                error
                            ))
                        })?;
                    match read_bounded_line(&mut stdout, line_limit) {
                        Ok(Some(line)) => Ok(line),
                        Ok(None) => Err(AicoreError::PluginFailed(
                            "plugin process exited".to_string(),
                        )),
                        Err(error) => {
                            Err(AicoreError::PluginFailed(format!("read failed: {}", error)))
                        },
                    }
                })();
                let failed = result.is_err();
                let _ = request.reply.send(result);
                if failed {
                    break;
                }
            }
        })
}

// Unix nonblocking pipes and Windows synchronous IO cancellation ensure cleanup
// can join its fixed two workers even if a descendant retained a pipe handle.
struct Interruptible<T> {
    stream: T,
    terminated: Arc<AtomicBool>,
}

impl<T: Read> Read for Interruptible<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.terminated.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "plugin IO stopped",
                ));
            }
            match self.stream.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                },
                result => return result,
            }
        }
    }
}

impl<T: Write> Write for Interruptible<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            if self.terminated.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "plugin IO stopped",
                ));
            }
            match self.stream.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                },
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(unix)]
fn configure_pipe<T: std::os::fd::AsRawFd>(pipe: &T) -> io::Result<()> {
    extern "C" {
        fn fcntl(fd: std::os::raw::c_int, command: std::os::raw::c_int, ...)
            -> std::os::raw::c_int;
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const NONBLOCK: i32 = 0x800;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const NONBLOCK: i32 = 0x4;
    // SAFETY: the borrowed pipe keeps this descriptor open; F_GETFL/F_SETFL
    // operate on this process-owned descriptor only and retain existing flags.
    let flags = unsafe { fcntl(pipe.as_raw_fd(), 3) };
    if flags == -1 || unsafe { fcntl(pipe.as_raw_fd(), 4, flags | NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn configure_pipe<T>(_: &T) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn cancel_pipe_io(worker: &JoinHandle<()>) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" {
        fn CancelSynchronousIo(thread: *mut std::ffi::c_void) -> i32;
    }
    // SAFETY: JoinHandle retains a valid thread handle until join below.
    // Cancellation affects only this host-owned worker's pending pipe IO.
    unsafe {
        CancelSynchronousIo(worker.as_raw_handle());
    }
}

struct PipeCancellation {
    #[cfg(windows)]
    handles: Mutex<Vec<std::os::windows::io::OwnedHandle>>,
}

impl PipeCancellation {
    fn new(
        stdin: &ChildStdin,
        stdout: &ChildStdout,
        stderr: Option<&ChildStderr>,
    ) -> io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsHandle;
            // Keep owned duplicates so cancellation never races a worker
            // closing a raw handle and that number being reused elsewhere.
            let mut handles = vec![
                stdin.as_handle().try_clone_to_owned()?,
                stdout.as_handle().try_clone_to_owned()?,
            ];
            if let Some(stderr) = stderr {
                handles.push(stderr.as_handle().try_clone_to_owned()?);
            }
            Ok(Self {
                handles: Mutex::new(handles),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (stdin, stdout, stderr);
            Ok(Self {})
        }
    }

    fn cancel(&self) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            #[link(name = "kernel32")]
            extern "system" {
                fn CancelIoEx(
                    file: *mut std::ffi::c_void,
                    overlapped: *mut std::ffi::c_void,
                ) -> i32;
            }
            let handles = self
                .handles
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for handle in handles.iter() {
                // SAFETY: these owned handles refer only to this child's pipes.
                // Null OVERLAPPED cancels all pending IO, including Rust's Windows
                // anonymous-pipe implementation's overlapped reads and writes.
                unsafe {
                    CancelIoEx(handle.as_raw_handle(), std::ptr::null_mut());
                }
            }
        }
    }

    fn close(&self) {
        #[cfg(windows)]
        self.handles
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

fn read_bounded_line<R: BufRead>(reader: &mut R, limit: usize) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let count = reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    if count > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin response exceeds MAX_LINE_BYTES",
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn spawn_stderr_drain(
    plugin_id: String,
    stderr: ChildStderr,
    terminated: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("aicore-plugin-stderr-{}", plugin_id))
        .spawn(move || {
            let mut reader = BufReader::new(Interruptible { stream: stderr, terminated: terminated.clone() });
            while !terminated.load(Ordering::Acquire) {
                match read_bounded_line(&mut reader, MAX_LINE_BYTES) {
                    Ok(Some(line)) if !line.trim().is_empty() => {
                        tracing::warn!(component="subprocess_plugin_host", plugin_id=%plugin_id, event="plugin_stderr", message=%line, "plugin stderr");
                    }
                    Ok(Some(_)) => {},
                    Ok(None) => break,
                    Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                        tracing::warn!(component="subprocess_plugin_host", plugin_id=%plugin_id, event="plugin_stderr_oversize", error=%e, "plugin stderr line rejected");
                    },
                    Err(e) => {
                        tracing::debug!(component="subprocess_plugin_host", plugin_id=%plugin_id, event="plugin_stderr_read_failed", error=%e, "plugin stderr drain stopped");
                        break;
                    }
                }
            }
        })
}
