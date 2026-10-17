//! Real subprocess pipe tests. The fixture performs no hardware operations.
use std::path::PathBuf;
use std::process::Command;
use std::sync::{mpsc, Arc, OnceLock};
use std::time::{Duration, Instant};

use aicore_contracts::errors::AicoreError;
use aicore_contracts::hardware::capability::HardwareDeviceKind;
use aicore_contracts::plugin::host::{IPluginHost, PluginHostState, PluginProcessSpec};
use aicore_contracts::traits::hardware_executor::HardwareExecutionHint;
use aicore_plugin::{HostRequest, HostResponse, SubprocessPluginHost};

const FIXTURE: &str = r#"
use std::io::{self, BufRead, Write};
use std::time::Duration;
fn main() {
    // Fail-safe even if a regression bypasses the host's timeout and test watchdog.
    std::thread::spawn(|| { std::thread::sleep(Duration::from_secs(10)); std::process::exit(91); });
    let name = std::env::current_exe().unwrap().file_stem().unwrap().to_string_lossy().to_string();
    if name.contains("nonreader") { std::thread::sleep(Duration::from_secs(9)); return; }
    if name.contains("memory-budget") {
        let mut bytes = Vec::<u8>::new();
        if bytes.try_reserve_exact(64 * 1024 * 1024).is_err() { std::process::exit(77); }
        bytes.resize(64 * 1024 * 1024, 1);
        std::hint::black_box(&bytes);
    }
    if name.contains("crash-exit") { std::process::exit(42); }
    if name == "retainer-parent" || name == "exit-parent" {
        let path = std::env::current_exe().unwrap();
        let child = std::process::Command::new(path.with_file_name(format!("nonreader-retainer{}", std::env::consts::EXE_SUFFIX)))
            .spawn().unwrap();
        std::fs::write(path.with_extension("pid"), child.id().to_string()).unwrap();
        if name == "retainer-parent" { let _ = io::stdin().lock().lines().next().unwrap().unwrap(); }
        return;
    }
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if name.contains("slow") { std::thread::sleep(Duration::from_millis(700)); }
        if name.contains("malformed") { println!("not-json"); }
        else if name.contains("oversized") { println!("{}", "x".repeat(65537)); }
        else if name.contains("error") { println!("{{\"kind\":\"error\",\"message\":\"fixture error\"}}"); }
        else if line.contains("shutdown") { println!("{{\"kind\":\"shutting_down\"}}"); return; }
        else if line.contains("execute") { println!("{{\"kind\":\"result\",\"output\":11}}"); }
        else { println!("{{\"kind\":\"pong\"}}"); }
        io::stdout().flush().unwrap();
    }
}
"#;

fn fixture(mode: &str) -> PathBuf {
    static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
    let directory = DIRECTORY.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("aicore-pipe-tests-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("fixture.rs");
        std::fs::write(&source, FIXTURE).unwrap();
        let executable = directory.join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
        let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "fixture build: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        directory
    });
    let path = directory.join(format!("{}{}", mode, std::env::consts::EXE_SUFFIX));
    if !path.exists() {
        std::fs::copy(
            directory.join(format!("fixture{}", std::env::consts::EXE_SUFFIX)),
            &path,
        )
        .unwrap();
    }
    path
}

fn spawn(
    host: &SubprocessPluginHost,
    mode: &str,
    timeout_ms: u64,
) -> aicore_contracts::plugin::host::PluginProcessHandle {
    let path = fixture(mode);
    host.spawn(&PluginProcessSpec {
        plugin_id: mode.into(),
        artifact_path: path.to_string_lossy().into(),
        timeout_ms,
        memory_limit_mb: 16,
    })
    .unwrap()
}

// This external watchdog kills only the exact child PID spawned by this test.
// It keeps RED finite even when the host is stuck before its own timeout.
struct NativeProcess {
    #[cfg(windows)]
    handle: std::os::windows::io::OwnedHandle,
    #[cfg(target_os = "linux")]
    fd: std::os::fd::OwnedFd,
}
impl NativeProcess {
    fn open(pid: u32) -> Self {
        #[cfg(windows)]
        {
            use std::os::windows::io::{FromRawHandle, OwnedHandle};
            use windows_sys::Win32::System::Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            };
            // SAFETY: the fixture is still owned/unreaped (or its descendant
            // has not yet been stopped). Retain identity instead of a bare PID.
            let raw = unsafe {
                OpenProcess(
                    PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    pid,
                )
            };
            assert!(!raw.is_null());
            Self {
                handle: unsafe { OwnedHandle::from_raw_handle(raw) },
            }
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::{FromRawFd, OwnedFd};
            // SAFETY: pidfd_open returns a new owned FD for this live fixture.
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            assert!(fd >= 0, "pidfd_open: {}", std::io::Error::last_os_error());
            Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd as i32) },
            }
        }
    }
    fn terminate(&self) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: this is the retained fixture handle, never a reused PID.
            unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(
                    self.handle.as_raw_handle(),
                    1,
                );
            }
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: pidfd_send_signal only addresses the retained identity.
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    self.fd.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                );
            }
        }
    }
    fn running(&self) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let mut code = 0;
            // SAFETY: valid owned query handle and output pointer.
            assert_ne!(
                unsafe {
                    windows_sys::Win32::System::Threading::GetExitCodeProcess(
                        self.handle.as_raw_handle(),
                        &mut code,
                    )
                },
                0
            );
            code == 259
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            let mut poll = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll borrows one valid pidfd; zero timeout is read-only.
            let result = unsafe { libc::poll(&mut poll, 1, 0) };
            assert!(result >= 0);
            result == 0
        }
    }
}

struct Watchdog(
    Option<mpsc::Sender<()>>,
    Option<std::thread::JoinHandle<()>>,
);
impl Watchdog {
    fn new(pid: u32) -> Self {
        let process = NativeProcess::open(pid);
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            if rx.recv_timeout(Duration::from_secs(2)).is_err() {
                process.terminate();
            }
        });
        Self(Some(tx), Some(worker))
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        let _ = self.0.take().unwrap().send(());
        let _ = self.1.take().unwrap().join();
    }
}

struct DescendantCleanup(NativeProcess);
impl Drop for DescendantCleanup {
    fn drop(&mut self) {
        self.0.terminate();
    }
}

fn large_request() -> HostRequest {
    // A maximum-size legal JSON payload plus its newline exceeds Windows'
    // 64 KiB anonymous pipe buffer, reproducing blocked delimiter writes too.
    let mut request = HostRequest::SetHardware {
        device_kind: HardwareDeviceKind::Gpu,
        device_id: String::new(),
        hint: HardwareExecutionHint::AutoMode,
    };
    let overhead = serde_json::to_vec(&request).unwrap().len();
    if let HostRequest::SetHardware { device_id, .. } = &mut request {
        *device_id = "x".repeat(65_536 - overhead);
    }
    request
}

#[test]
fn nonreading_child_write_is_inside_call_deadline() {
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "nonreader-write", 150);
    let _watchdog = Watchdog::new(handle.pid);
    let start = Instant::now();
    let error = host.call(&handle, &large_request()).unwrap_err();
    assert!(
        matches!(error, AicoreError::Timeout(_)),
        "expected call deadline, got {error}"
    );
    assert!(
        start.elapsed() < Duration::from_millis(900),
        "write escaped call deadline: {:?}",
        start.elapsed()
    );
    assert_eq!(host.state(&handle), PluginHostState::Crashed);
    assert_eq!(host.recent_crashes().len(), 1);
    host.stop(&handle).unwrap();
    assert_eq!(host.state(&handle), PluginHostState::Stopped);
}

#[test]
fn blocked_plugin_does_not_block_independent_plugin_or_state() {
    let host = Arc::new(SubprocessPluginHost::new());
    let blocked = spawn(&host, "nonreader-independent", 1000);
    let healthy = spawn(&host, "healthy-independent", 1000);
    let _watchdog = Watchdog::new(blocked.pid);
    let worker_host = host.clone();
    let worker_handle = blocked.clone();
    let worker = std::thread::spawn(move || worker_host.call(&worker_handle, &large_request()));
    std::thread::sleep(Duration::from_millis(60));
    let start = Instant::now();
    assert_eq!(host.state(&healthy), PluginHostState::Running);
    assert!(matches!(
        host.call(&healthy, &HostRequest::Ping).unwrap(),
        HostResponse::Pong
    ));
    let elapsed = start.elapsed();
    let _ = worker.join().unwrap();
    host.stop(&blocked).unwrap();
    host.stop(&healthy).unwrap();
    assert!(
        elapsed < Duration::from_millis(500),
        "independent plugin waited for blocked IO: {elapsed:?}"
    );
}

#[test]
fn same_plugin_queue_wait_counts_toward_deadline() {
    let host = Arc::new(SubprocessPluginHost::new());
    let handle = spawn(&host, "slow-queue", 1000);
    let _watchdog = Watchdog::new(handle.pid);
    let worker_host = host.clone();
    let worker_handle = handle.clone();
    let worker = std::thread::spawn(move || worker_host.call(&worker_handle, &HostRequest::Ping));
    std::thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    let result = host.call(&handle, &HostRequest::Execute { input: 5 });
    let elapsed = start.elapsed();
    assert!(matches!(
        worker.join().unwrap().unwrap(),
        HostResponse::Pong
    ));
    host.stop(&handle).unwrap();
    assert!(
        matches!(result, Err(AicoreError::Timeout(_))),
        "queued call escaped deadline: {result:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1300),
        "queue deadline restarted: {elapsed:?}"
    );
}

#[test]
fn stop_interrupts_blocked_write_without_waiting_for_call_timeout() {
    let host = Arc::new(SubprocessPluginHost::new());
    let handle = spawn(&host, "nonreader-stop", 5000);
    let _watchdog = Watchdog::new(handle.pid);
    let worker_host = host.clone();
    let worker_handle = handle.clone();
    let worker = std::thread::spawn(move || worker_host.call(&worker_handle, &large_request()));
    std::thread::sleep(Duration::from_millis(60));
    let start = Instant::now();
    host.stop(&handle).unwrap();
    let elapsed = start.elapsed();
    assert!(worker.join().unwrap().is_err());
    assert_eq!(host.state(&handle), PluginHostState::Stopped);
    assert!(
        elapsed < Duration::from_millis(900),
        "stop blocked on pending write: {elapsed:?}"
    );
}

#[test]
fn successful_calls_restore_io_and_keep_responses_serialized() {
    let host = Arc::new(SubprocessPluginHost::new());
    let handle = spawn(&host, "healthy-serial", 1000);
    assert!(matches!(
        host.call(&handle, &large_request()).unwrap(),
        HostResponse::Pong
    ));
    let mut workers = Vec::new();
    for i in 0..8 {
        let host = host.clone();
        let handle = handle.clone();
        workers.push(std::thread::spawn(move || {
            if i % 2 == 0 {
                assert!(matches!(
                    host.call(&handle, &HostRequest::Ping).unwrap(),
                    HostResponse::Pong
                ));
            } else {
                assert!(matches!(
                    host.call(&handle, &HostRequest::Execute { input: 5 })
                        .unwrap(),
                    HostResponse::Result { output: 11 }
                ));
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(host.state(&handle), PluginHostState::Running);
    assert!(host.recent_crashes().is_empty());
    host.stop(&handle).unwrap();
}

#[test]
fn malformed_and_plugin_error_responses_preserve_reusable_transport() {
    let host = SubprocessPluginHost::new();
    let malformed = spawn(&host, "malformed-response", 1000);
    for _ in 0..2 {
        let error = host.call(&malformed, &HostRequest::Ping).unwrap_err();
        assert!(error.to_string().contains("invalid response"), "{error}");
    }
    assert_eq!(host.state(&malformed), PluginHostState::Running);
    host.stop(&malformed).unwrap();
    let handle = spawn(&host, "error-response", 1000);
    for _ in 0..2 {
        assert!(
            matches!(host.call(&handle, &HostRequest::Ping).unwrap(), HostResponse::Error { message } if message == "fixture error")
        );
    }
    assert!(host.recent_crashes().is_empty());
    host.stop(&handle).unwrap();
}

#[test]
fn oversized_request_is_rejected_before_touching_transport() {
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "healthy-oversize-request", 1000);
    let mut request = large_request();
    if let HostRequest::SetHardware { device_id, .. } = &mut request {
        device_id.push('x');
    }
    assert!(matches!(
        host.call(&handle, &request),
        Err(AicoreError::InvalidRequest(_))
    ));
    assert!(matches!(
        host.call(&handle, &HostRequest::Ping).unwrap(),
        HostResponse::Pong
    ));
    host.stop(&handle).unwrap();
}

#[test]
fn oversized_response_kills_child_and_records_one_crash() {
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "oversized-response", 1000);
    let error = host.call(&handle, &HostRequest::Ping).unwrap_err();
    assert!(error.to_string().contains("response exceeds"), "{error}");
    assert_eq!(host.state(&handle), PluginHostState::Crashed);
    assert_eq!(host.recent_crashes().len(), 1);
    host.stop(&handle).unwrap();
}

#[test]
fn inherited_pipe_handles_do_not_keep_host_workers_alive() {
    fixture("nonreader-retainer");
    let parent = fixture("retainer-parent");
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "retainer-parent", 150);
    let _watchdog = Watchdog::new(handle.pid);
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    while !parent.with_extension("pid").exists() {
        assert!(
            Instant::now() < ready_deadline,
            "descendant did not become ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let pid = std::fs::read_to_string(parent.with_extension("pid"))
        .unwrap()
        .parse()
        .unwrap();
    let _cleanup = DescendantCleanup(NativeProcess::open(pid));
    let start = Instant::now();
    let result = host.call(&handle, &HostRequest::Ping);
    let elapsed = start.elapsed();
    assert!(matches!(result, Err(AicoreError::Timeout(_))), "{result:?}");
    assert!(
        elapsed < Duration::from_millis(900),
        "inherited pipe retained worker: {elapsed:?}"
    );
    host.stop(&handle).unwrap();
}

#[cfg(any(windows, target_os = "linux"))]
fn child_is_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
            fn GetExitCodeProcess(process: *mut c_void, exit_code: *mut u32) -> i32;
            fn CloseHandle(handle: *mut c_void) -> i32;
        }
        // SAFETY: query-only access to the PID just created by this test; any
        // obtained handle is closed before returning.
        unsafe {
            let process = OpenProcess(0x1000, 0, pid);
            if process.is_null() {
                return false;
            }
            let mut code = 0;
            let success = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            assert_ne!(success, 0);
            code == 259
        }
    }
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn dropping_host_reaps_running_children() {
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "nonreader-drop", 1000);
    let _watchdog = Watchdog::new(handle.pid);
    assert!(child_is_running(handle.pid));
    let start = Instant::now();
    drop(host);
    assert!(
        !child_is_running(handle.pid),
        "host drop left a child running"
    );
    assert!(start.elapsed() < Duration::from_millis(900));
}

#[cfg(windows)]
fn resource_counts() -> (u32, usize) {
    use std::ffi::c_void;
    #[repr(C)]
    struct ThreadEntry {
        size: u32,
        usage: u32,
        thread: u32,
        owner: u32,
        base_priority: i32,
        delta_priority: i32,
        flags: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessHandleCount(process: *mut c_void, count: *mut u32) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
        fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    // SAFETY: these calls only query this test's process/thread counters. The
    // snapshot is checked for failure and closed before returning.
    unsafe {
        let mut handles = 0;
        assert_ne!(GetProcessHandleCount(GetCurrentProcess(), &mut handles), 0);
        let snapshot = CreateToolhelp32Snapshot(4, 0);
        assert_ne!(snapshot as isize, -1);
        let mut entry = ThreadEntry {
            size: std::mem::size_of::<ThreadEntry>() as u32,
            usage: 0,
            thread: 0,
            owner: 0,
            base_priority: 0,
            delta_priority: 0,
            flags: 0,
        };
        let mut count = 0;
        let mut found = Thread32First(snapshot, &mut entry);
        while found != 0 {
            if entry.owner == std::process::id() {
                count += 1;
            }
            found = Thread32Next(snapshot, &mut entry);
        }
        assert_ne!(CloseHandle(snapshot), 0);
        (handles, count)
    }
}

#[cfg(target_os = "linux")]
fn observable_thread_count(entries: &[std::fs::DirEntry]) -> usize {
    // getdents may retain a TID that disappeared during enumeration. Verify
    // every entry against the kernel; still-readable threads (including Z/X)
    // are counted, and unexpected read failures must fail the probe.
    entries
        .iter()
        .filter(
            |entry| match std::fs::read_to_string(entry.path().join("stat")) {
                Ok(_) => true,
                Err(error)
                    if matches!(error.raw_os_error(), Some(libc::ENOENT) | Some(libc::ESRCH)) =>
                {
                    false
                },
                Err(error) => panic!(
                    "cannot verify observed thread {:?}: {error}",
                    entry.file_name()
                ),
            },
        )
        .count()
}

#[cfg(target_os = "linux")]
#[test]
fn resource_observation_rejects_stale_entries_but_counts_live_threads() {
    let (ready, started) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        // SAFETY: gettid queries the calling test thread only.
        ready
            .send(unsafe { libc::syscall(libc::SYS_gettid) }.to_string())
            .unwrap();
        wait.recv().unwrap();
    });
    let tid = started.recv_timeout(Duration::from_secs(2)).unwrap();
    let entries: Vec<_> = std::fs::read_dir("/proc/self/task")
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_name().to_string_lossy() == tid)
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        observable_thread_count(&entries),
        1,
        "a live worker must never be filtered out"
    );
    release.send(()).unwrap();
    worker.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while entries[0].path().exists() {
        assert!(
            Instant::now() < deadline,
            "joined native thread was not reclaimed"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        observable_thread_count(&entries),
        0,
        "directory enumeration is not a live resource snapshot"
    );
}

#[cfg(target_os = "linux")]
fn resource_counts() -> (u32, usize) {
    let entries: Vec<_> = std::fs::read_dir("/proc/self/task")
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let threads = observable_thread_count(&entries);
    drop(entries); // Do not charge this observation's directory FD to the host.
    (
        std::fs::read_dir("/proc/self/fd").unwrap().count() as u32,
        threads,
    )
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn repeated_timeouts_release_workers_and_handles() {
    // Isolate the counter assertions from other concurrently running tests.
    if std::env::var_os("AICORE_PIPE_RESOURCE_PROBE").is_none() {
        let result = Command::new(std::env::current_exe().unwrap())
            .env("AICORE_PIPE_RESOURCE_PROBE", "1")
            .args([
                "--exact",
                "repeated_timeouts_release_workers_and_handles",
                "--nocapture",
            ])
            .output()
            .unwrap();
        eprintln!("{}", String::from_utf8_lossy(&result.stderr));
        assert!(
            result.status.success(),
            "resource probe:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    fixture("nonreader-resources");
    // Initialize Rust/Windows' lazily allocated process/pipe/thread facilities
    // before measuring repeated lifecycle growth.
    {
        let host = SubprocessPluginHost::new();
        let handle = spawn(&host, "healthy-resource-warmup", 1000);
        assert!(matches!(
            host.call(&handle, &HostRequest::Ping).unwrap(),
            HostResponse::Pong
        ));
        host.stop(&handle).unwrap();
    }
    let baseline = resource_counts();
    for _ in 0..12 {
        let host = SubprocessPluginHost::new();
        let handle = spawn(&host, "nonreader-resources", 100);
        let _watchdog = Watchdog::new(handle.pid);
        assert!(matches!(
            host.call(&handle, &large_request()),
            Err(AicoreError::Timeout(_))
        ));
        assert_eq!(host.state(&handle), PluginHostState::Crashed);
        // Repeated failed calls cannot create new workers or duplicate events.
        assert!(host.call(&handle, &HostRequest::Ping).is_err());
        assert_eq!(host.recent_crashes().len(), 1);
        host.stop(&handle).unwrap();
        drop(host);
        drop(_watchdog);
        let after = resource_counts();
        eprintln!("resource lifecycle baseline={baseline:?}, after={after:?}");
        assert!(
            after.0 <= baseline.0,
            "leaked handles: baseline={baseline:?}, after={after:?}"
        );
        assert!(
            after.1 <= baseline.1,
            "leaked workers: baseline={baseline:?}, after={after:?}"
        );
    }
}

#[test]
fn os_memory_budget_prevents_allocation_and_host_survives() {
    let host = SubprocessPluginHost::new();
    let limited = spawn(&host, "memory-budget", 1000);
    let result = host.call(&limited, &HostRequest::Ping);
    assert!(
        result.is_err(),
        "64 MiB allocation escaped 16 MiB OS budget: {result:?}"
    );
    assert_eq!(host.state(&limited), PluginHostState::Crashed);
    host.stop(&limited).unwrap();
    let healthy = spawn(&host, "healthy-after-memory", 1000);
    assert!(matches!(
        host.call(&healthy, &HostRequest::Ping).unwrap(),
        HostResponse::Pong
    ));
    host.stop(&healthy).unwrap();
}

#[test]
fn unrepresentable_memory_budget_is_rejected_before_execution() {
    let host = SubprocessPluginHost::new();
    assert!(
        host.spawn(&PluginProcessSpec {
            plugin_id: "overflow".into(),
            artifact_path: fixture("healthy-overflow").to_string_lossy().into(),
            timeout_ms: 1000,
            memory_limit_mb: u64::MAX,
        })
        .is_err(),
        "unenforceable limit must fail closed"
    );
}

#[test]
fn abnormal_exit_is_reaped_without_killing_host_or_other_plugins() {
    let host = SubprocessPluginHost::new();
    let crashed = spawn(&host, "crash-exit", 1000);
    assert!(host.call(&crashed, &HostRequest::Ping).is_err());
    assert_eq!(host.state(&crashed), PluginHostState::Crashed);
    host.stop(&crashed).unwrap();
    assert!(!child_is_running(crashed.pid));
    let healthy = spawn(&host, "healthy-after-crash", 1000);
    assert!(matches!(
        host.call(&healthy, &HostRequest::Ping).unwrap(),
        HostResponse::Pong
    ));
    host.stop(&healthy).unwrap();
}

#[test]
fn natural_parent_exit_also_reclaims_descendants() {
    fixture("nonreader-retainer");
    let parent = fixture("exit-parent");
    let host = SubprocessPluginHost::new();
    let handle = spawn(&host, "exit-parent", 1000);
    let deadline = Instant::now() + Duration::from_secs(2);
    let pid_path = parent.with_extension("pid");
    let pid = loop {
        if let Ok(value) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = value.parse::<u32>() {
                break pid;
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    let descendant = DescendantCleanup(NativeProcess::open(pid));
    while host.state(&handle) == PluginHostState::Running {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(host.state(&handle), PluginHostState::Stopped);
    while descendant.0.running() {
        assert!(
            Instant::now() < deadline,
            "terminal parent left a descendant running"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    host.stop(&handle).unwrap();
}
