/// Identity observed by the IPC transport, never supplied by the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    pub fn process_id(self) -> Option<u32> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux { pid, .. } => u32::try_from(pid).ok().filter(|p| *p > 0),
            #[cfg(target_os = "windows")]
            Self::Windows { pid, .. } => Some(pid).filter(|p| *p > 0),
            Self::Unauthenticated | Self::Internal => None,
        }
    }
    /// The authenticated OS principal, never a field supplied by a client.
    pub fn principal(self) -> Option<String> {
        if !self.is_current() {
            return None;
        }
        let principal = match self {
            #[cfg(target_os = "linux")]
            Self::Linux { uid, .. } => Some(format!("uid:{uid}")),
            #[cfg(target_os = "windows")]
            Self::Windows { pid, .. } => crate::transport::process_user_sid(pid),
            Self::Unauthenticated | Self::Internal => None,
        };
        if self.is_current() {
            principal
        } else {
            None
        }
    }
    /// Read-only executable digest for an operator-pinned application policy.
    /// A missing/inaccessible executable fails closed. This is not a code
    /// signing or Windows ACL certification claim.
    pub fn artifact_sha256(self) -> Option<String> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        if !self.is_current() {
            return None;
        }
        let path = match self {
            #[cfg(target_os = "linux")]
            Self::Linux {
                pid,
                executable_dev,
                executable_ino,
                ..
            } => {
                use std::os::unix::fs::MetadataExt;
                let path = std::path::PathBuf::from(format!("/proc/{pid}/exe"));
                let metadata = std::fs::metadata(&path).ok()?;
                if (metadata.dev(), metadata.ino()) != (executable_dev, executable_ino) {
                    return None;
                }
                path
            },
            #[cfg(target_os = "windows")]
            Self::Windows { pid, .. } => crate::transport::process_executable_path(pid)?,
            Self::Unauthenticated | Self::Internal => return None,
        };
        let mut file = std::fs::File::open(path).ok()?;
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            bytes = bytes.checked_add(count as u64)?;
            if bytes > 256 * 1024 * 1024 {
                return None;
            }
            hasher.update(&buffer[..count]);
        }
        self.is_current().then(|| hex::encode(hasher.finalize()))
    }
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

#[cfg(all(test, target_os = "windows"))]
mod windows_artifact_tests {
    use super::*;

    #[test]
    fn current_process_executable_has_read_only_digest() {
        let pid = std::process::id();
        let created_at_100ns = crate::transport::process_creation_time(pid).unwrap();
        let peer = ClientIdentity::Windows {
            pid,
            created_at_100ns,
        };
        let digest = peer.artifact_sha256().unwrap();
        assert!(peer.principal().unwrap().starts_with("S-1-"));
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(ClientIdentity::Unauthenticated.artifact_sha256(), None);
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
