use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::priority::PolicyPriority,
    traits::process_controller::{AffinityMask, IProcessController},
};

/// Linux process controller.
///
/// Use `setpriority(2)` with `sched_setaffinity(2)`.
pub struct LinuxProcessController;

impl LinuxProcessController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxProcessController {
    fn default() -> Self {
        Self::new()
    }
}

impl IProcessController for LinuxProcessController {
    fn set_priority(&self, pid: u32, priority: PolicyPriority) -> ContractResult<()> {
        let nice = priority_to_nice(priority);
        let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };
        if rc == 0 {
            return Ok(());
        }
        let errno = std::io::Error::last_os_error();
        match errno.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => Err(ContractError::PermissionDenied(
                format!("setpriority(pid={}, nice={}) denied: {}", pid, nice, errno),
            )),
            Some(libc::ESRCH) => Err(ContractError::ResourceUnavailable(format!(
                "no such process: pid={}",
                pid
            ))),
            _ => Err(ContractError::AdapterUnavailable(format!(
                "setpriority failed: {}",
                errno
            ))),
        }
    }

    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority> {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t) };
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error().unwrap_or(0) != 0 {
            return Err(ContractError::ResourceUnavailable(format!(
                "getpriority(pid={}) failed: {}",
                pid, errno
            )));
        }
        Ok(nice_to_priority(nice))
    }

    fn set_affinity(&self, pid: u32, mask: &AffinityMask) -> ContractResult<()> {
        if mask.is_empty() {
            return Err(ContractError::InvalidRequest(
                "affinity mask is empty".to_string(),
            ));
        }

        let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::CPU_ZERO(&mut set);
        }

        let max_cpu = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_CONF) } as u32;
        for &core in &mask.cores {
            if core >= max_cpu {
                return Err(ContractError::InvalidRequest(format!(
                    "core {} out of range (max {})",
                    core, max_cpu
                )));
            }
            unsafe {
                libc::CPU_SET(core as usize, &mut set);
            }
        }

        let rc = unsafe {
            libc::sched_setaffinity(
                pid as libc::pid_t,
                std::mem::size_of::<libc::cpu_set_t>(),
                &set,
            )
        };

        if rc == 0 {
            return Ok(());
        }

        let errno = std::io::Error::last_os_error();
        match errno.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => Err(ContractError::PermissionDenied(
                format!("sched_setaffinity(pid={}) denied: {}", pid, errno),
            )),
            Some(libc::ESRCH) => Err(ContractError::ResourceUnavailable(format!(
                "no such process: pid={}",
                pid
            ))),
            Some(libc::EINVAL) => Err(ContractError::InvalidRequest(format!(
                "invalid affinity for pid={}: {}",
                pid, errno
            ))),
            _ => Err(ContractError::AdapterUnavailable(format!(
                "sched_setaffinity failed: {}",
                errno
            ))),
        }
    }

    fn get_affinity(&self, pid: u32) -> ContractResult<AffinityMask> {
        let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::sched_getaffinity(
                pid as libc::pid_t,
                std::mem::size_of::<libc::cpu_set_t>(),
                &mut set,
            )
        };

        if rc != 0 {
            let errno = std::io::Error::last_os_error();
            return match errno.raw_os_error() {
                Some(libc::ESRCH) => Err(ContractError::ResourceUnavailable(format!(
                    "no such process: pid={}",
                    pid
                ))),
                _ => Err(ContractError::AdapterUnavailable(format!(
                    "sched_getaffinity failed: {}",
                    errno
                ))),
            };
        }

        let max_cpu = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_CONF) } as u32;
        let mut cores = Vec::new();
        for core in 0..max_cpu {
            if unsafe { libc::CPU_ISSET(core as usize, &set) } {
                cores.push(core);
            }
        }

        Ok(AffinityMask::new(cores))
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "process.priority.read",
            "process.priority.write",
            "process.affinity.read",
            "process.affinity.write",
        ]
    }

    fn platform_name(&self) -> &'static str {
        "linux"
    }
}

fn priority_to_nice(p: PolicyPriority) -> libc::c_int {
    match p {
        PolicyPriority::Background => 19,
        PolicyPriority::Low => 10,
        PolicyPriority::Normal => 0,
        PolicyPriority::High => -5,
        PolicyPriority::Interactive => -10,
    }
}

fn nice_to_priority(nice: libc::c_int) -> PolicyPriority {
    if nice >= 15 {
        PolicyPriority::Background
    } else if nice >= 5 {
        PolicyPriority::Low
    } else if nice >= -2 {
        PolicyPriority::Normal
    } else if nice >= -8 {
        PolicyPriority::High
    } else {
        PolicyPriority::Interactive
    }
}
