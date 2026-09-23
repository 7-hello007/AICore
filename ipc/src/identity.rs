/// Identity observed by the IPC transport, never supplied by the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientIdentity {
    /// Linux SO_PEERCRED plus process start tick observed at accept time.
    #[cfg(target_os = "linux")]
    Linux {
        uid: u32,
        pid: i32,
        start_time_ticks: u64,
    },
    /// Windows named-pipe client process ID and creation time. Pipe ACL also
    /// restricts local users; PID reuse cannot inherit an existing binding.
    #[cfg(target_os = "windows")]
    Windows { pid: u32, created_at_100ns: u64 },
    /// Transport could not establish an OS peer identity; mutating actions fail.
    Unauthenticated,
    /// Trusted in-process caller used by direct API and isolated tests only.
    Internal,
}
