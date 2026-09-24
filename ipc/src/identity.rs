/// Identity observed by the IPC transport, never supplied by the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientIdentity {
    /// Linux SO_PEERCRED plus process start tick observed at accept time.
    #[cfg(target_os = "linux")]
    Linux {
        uid: u32,
        pid: i32,
        start_time_ticks: u64,
        executable_dev: u64,
        executable_ino: u64,
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

impl ClientIdentity {
    /// Recheck the live peer before an application-scoped request. An
    /// inaccessible peer is not accepted, but also is not automatically
    /// reclaimed by the stale-owner cleanup path.
    pub fn is_current(self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux {
                uid,
                pid,
                start_time_ticks,
                executable_dev,
                executable_ino,
            } => {
                use std::os::unix::fs::MetadataExt;
                std::fs::metadata(format!("/proc/{pid}"))
                    .ok()
                    .is_some_and(|metadata| metadata.uid() == uid)
                    && crate::transport::read_process_start_time(pid).ok() == Some(start_time_ticks)
                    && crate::transport::read_process_executable_identity(pid).ok()
                        == Some((executable_dev, executable_ino))
            },
            #[cfg(target_os = "windows")]
            Self::Windows {
                pid,
                created_at_100ns,
            } => crate::transport::process_creation_time(pid) == Some(created_at_100ns),
            Self::Unauthenticated => false,
            Self::Internal => true,
        }
    }

    /// Only a confirmed process disappearance or identity mismatch permits
    /// reclaiming an application binding. Permission/read errors fail closed.
    pub fn is_stale(self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux {
                uid,
                pid,
                start_time_ticks,
                executable_dev,
                executable_ino,
            } => {
                use std::os::unix::fs::MetadataExt;
                let path = format!("/proc/{pid}");
                match std::fs::metadata(path) {
                    Ok(metadata) if metadata.uid() != uid => true,
                    Ok(_) => match crate::transport::read_process_start_time(pid) {
                        Ok(current) if current != start_time_ticks => true,
                        Ok(_) => match crate::transport::read_process_executable_identity(pid) {
                            Ok(current) => current != (executable_dev, executable_ino),
                            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                        },
                        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                    },
                    Err(error) => error.kind() == std::io::ErrorKind::NotFound,
                }
            },
            #[cfg(target_os = "windows")]
            Self::Windows {
                pid,
                created_at_100ns,
            } => crate::transport::process_identity_stale(pid, created_at_100ns),
            Self::Unauthenticated | Self::Internal => false,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn executable_change_invalidates_live_peer_identity() {
        let pid = std::process::id() as i32;
        let uid = std::fs::metadata(format!("/proc/{pid}")).unwrap().uid();
        let start_time_ticks = crate::transport::read_process_start_time(pid).unwrap();
        let (executable_dev, executable_ino) =
            crate::transport::read_process_executable_identity(pid).unwrap();
        let current = ClientIdentity::Linux {
            uid,
            pid,
            start_time_ticks,
            executable_dev,
            executable_ino,
        };
        assert!(current.is_current());
        assert!(!current.is_stale());
        let replaced = ClientIdentity::Linux {
            uid,
            pid,
            start_time_ticks,
            executable_dev,
            executable_ino: executable_ino.wrapping_add(1),
        };
        assert!(!replaced.is_current());
        assert!(replaced.is_stale());
    }
}
