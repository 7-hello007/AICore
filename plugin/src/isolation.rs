use std::sync::atomic::{AtomicU32, Ordering};

use aicore_contracts::plugin::state::PluginState;

/// OS resource containment for a directly owned subprocess and its descendants.
/// This is not a sandbox against same-user filesystem or network access.
pub(crate) struct ProcessIsolation {
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
    #[cfg(target_os = "linux")]
    group: std::sync::atomic::AtomicI32,
}

impl ProcessIsolation {
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn group_reserved(&self) -> bool {
        self.group.load(Ordering::Acquire) != 0
    }
    /// No child code runs before the limit is installed. Failure kills/reaps
    /// the still-suspended Windows child, or fails Linux's pre-exec handshake.
    pub fn spawn(
        command: &mut std::process::Command,
        memory_mb: u64,
    ) -> std::io::Result<(std::process::Child, Self)> {
        let bytes = memory_mb
            .checked_mul(1024 * 1024)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unenforceable memory limit",
                )
            })?;
        #[cfg(windows)]
        {
            windows_limit::spawn(command, bytes)
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: pre_exec performs only async-signal-safe OS operations;
            // the hard limit and private process group precede exec/user code.
            unsafe {
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: bytes as libc::rlim_t,
                        rlim_max: bytes as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 || libc::setpgid(0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    let mut readback = libc::rlimit {
                        rlim_cur: 0,
                        rlim_max: 0,
                    };
                    if libc::getrlimit(libc::RLIMIT_AS, &mut readback) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if readback.rlim_cur != limit.rlim_cur || readback.rlim_max != limit.rlim_max {
                        return Err(std::io::Error::from_raw_os_error(libc::EPERM));
                    }
                    Ok(())
                });
            }
            let child = command.spawn()?;
            let group = child.id() as i32;
            Ok((
                child,
                Self {
                    group: std::sync::atomic::AtomicI32::new(group),
                },
            ))
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = (command, bytes);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "OS memory containment unavailable",
            ))
        }
    }

    /// Caller holds the child lock. Never reap the group leader before this
    /// clears/kills the group: an integer PGID is not a lifetime-bound handle.
    pub(crate) fn try_wait(
        &self,
        child: &mut std::process::Child,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        #[cfg(target_os = "linux")]
        if self.group.load(Ordering::Acquire) != 0 {
            // SAFETY: WNOWAIT observes only our owned child's exit without
            // releasing its PID. The child lock serializes all reaping paths.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result != 0 {
                self.group.store(0, Ordering::Release);
                return Err(std::io::Error::last_os_error());
            }
            if unsafe { info.si_pid() } == 0 {
                return Ok(None);
            }
            self.terminate();
        }
        let status = child.try_wait()?;
        if status.is_some() {
            self.terminate();
        }
        Ok(status)
    }

    pub(crate) fn terminate(&self) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: job is owned and never inherited by the child.
            unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(
                    self.job.as_raw_handle(),
                    1,
                );
            }
        }
        #[cfg(target_os = "linux")]
        // SAFETY: this private group was created for our owned child only.
        {
            let group = self.group.swap(0, Ordering::AcqRel);
            if group > 0 {
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            }
        }
    }
}

impl Drop for ProcessIsolation {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod lifecycle_tests {
    use super::*;
    use std::{os::unix::process::CommandExt, process::Command};

    #[test]
    fn already_reaped_leader_cannot_signal_a_reused_group() {
        let (mut leader, isolation) =
            ProcessIsolation::spawn(&mut Command::new("/bin/true"), 16).unwrap();
        leader.wait().unwrap(); // Inject loss of the lifetime reservation.
        let mut command = Command::new("/bin/sleep");
        command.arg("5");
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
        let mut unrelated = command.spawn().unwrap();
        // Model a numeric PGID having been reused by another live process.
        // The child under test is already reaped, so no signal is authorized.
        isolation
            .group
            .store(unrelated.id() as i32, Ordering::Release);
        let observed = isolation.try_wait(&mut leader);
        let cleared = !isolation.group_reserved();
        drop(isolation);
        let survived = unrelated.try_wait().unwrap().is_none();
        let _ = unrelated.kill();
        unrelated.wait().unwrap();
        assert!(observed.is_err());
        assert!(cleared);
        assert!(survived, "stale group identity killed an unrelated process");
    }
}

#[cfg(windows)]
mod windows_limit {
    use super::ProcessIsolation;
    use std::{
        io,
        mem::{size_of, zeroed},
        os::windows::{
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
            process::CommandExt,
        },
        process::{Child, Command},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{Diagnostics::ToolHelp::*, JobObjects::*, Threading::*},
    };

    pub(super) fn spawn(
        command: &mut Command,
        bytes: usize,
    ) -> io::Result<(Child, ProcessIsolation)> {
        spawn_with_assignment(command, bytes, |job, process| {
            // SAFETY: handles refer to the job and the suspended owned child.
            unsafe { AssignProcessToJobObject(job, process) != 0 }
        })
    }

    fn spawn_with_assignment(
        command: &mut Command,
        bytes: usize,
        assign: impl FnOnce(
            windows_sys::Win32::Foundation::HANDLE,
            windows_sys::Win32::Foundation::HANDLE,
        ) -> bool,
    ) -> io::Result<(Child, ProcessIsolation)> {
        // SAFETY: all handles are owned, all structures have the Windows ABI;
        // the child is suspended until job membership and limits are read back.
        unsafe {
            let raw = CreateJobObjectW(ptr::null(), ptr::null());
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = OwnedHandle::from_raw_handle(raw);
            let mut limit: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_JOB_MEMORY
                | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            limit.ProcessMemoryLimit = bytes;
            limit.JobMemoryLimit = bytes;
            let size = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if SetInformationJobObject(
                raw,
                JobObjectExtendedLimitInformation,
                &limit as *const _ as _,
                size,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let mut readback: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            if QueryInformationJobObject(
                raw,
                JobObjectExtendedLimitInformation,
                &mut readback as *mut _ as _,
                size,
                ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            if readback.ProcessMemoryLimit != bytes
                || readback.JobMemoryLimit != bytes
                || readback.BasicLimitInformation.LimitFlags
                    != limit.BasicLimitInformation.LimitFlags
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "job limit readback mismatch",
                ));
            }
            let mut child = command
                .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
                .spawn()?;
            let setup = (|| {
                if !assign(raw, child.as_raw_handle()) {
                    return Err(io::Error::last_os_error());
                }
                let mut member = 0;
                if IsProcessInJob(child.as_raw_handle(), raw, &mut member) == 0 || member == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "job membership not verified",
                    ));
                }
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                if snapshot == INVALID_HANDLE_VALUE {
                    return Err(io::Error::last_os_error());
                }
                let _snapshot = OwnedHandle::from_raw_handle(snapshot);
                let mut entry: THREADENTRY32 = zeroed();
                entry.dwSize = size_of::<THREADENTRY32>() as u32;
                let mut found = Thread32First(snapshot, &mut entry);
                while found != 0 {
                    if entry.th32OwnerProcessID == child.id() {
                        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        if thread.is_null() {
                            return Err(io::Error::last_os_error());
                        }
                        let _thread = OwnedHandle::from_raw_handle(thread);
                        if ResumeThread(thread) != 1 {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                "unexpected primary thread suspend count",
                            ));
                        }
                        return Ok(());
                    }
                    found = Thread32Next(snapshot, &mut entry);
                }
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "suspended primary thread not found",
                ))
            })();
            if let Err(error) = setup {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok((child, ProcessIsolation { job }))
        }
    }

    #[cfg(test)]
    #[test]
    fn assignment_denial_kills_suspended_child_before_entry_point() {
        use std::os::windows::io::BorrowedHandle;
        let mut retained = None;
        let mut command = Command::new("cmd.exe");
        command
            .args(["/c", "exit", "77"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let result = spawn_with_assignment(&mut command, 16 * 1024 * 1024, |_, process| {
            // SAFETY: the child handle remains live during this closure. Force
            // an actual OS assignment error, retaining a query handle for proof.
            unsafe {
                retained = Some(
                    BorrowedHandle::borrow_raw(process)
                        .try_clone_to_owned()
                        .unwrap(),
                );
                AssignProcessToJobObject(INVALID_HANDLE_VALUE, process) != 0
            }
        });
        assert!(result.is_err());
        let process = retained.unwrap();
        let mut code = 0;
        // SAFETY: retained owns a valid process handle even after child reap.
        assert_ne!(
            unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) },
            0
        );
        assert_ne!(code, 259, "denied child left running");
        assert_ne!(code, 77, "child entry point ran before containment");
    }
}

/// The wrong border for a single plugin.
///
/// Semantics:
/// - Record failure.
/// - Auto-set Failed beyond the threshold.
/// - affect only the plugin and not other plugins or Core.
#[derive(Debug)]
pub struct PluginIsolation {
    name: String,
    failure_threshold: u32,
    failure_count: AtomicU32,
}

impl PluginIsolation {
    pub fn new(name: impl Into<String>, failure_threshold: u32) -> Self {
        Self {
            name: name.into(),
            failure_threshold: failure_threshold.max(1),
            failure_count: AtomicU32::new(0),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn failure_threshold(&self) -> u32 {
        self.failure_threshold
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Record a failure. Should return be isolated (to Failed).
    pub fn record_failure(&self) -> bool {
        let n = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        n >= self.failure_threshold
    }

    /// Record a success. Reset the failure count.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
    }

    /// Next PluginState is recommended according to current status.
    pub fn recommended_state(&self, current: PluginState) -> PluginState {
        if current == PluginState::Active && self.failure_count() >= self.failure_threshold {
            PluginState::Failed
        } else {
            current
        }
    }
}
