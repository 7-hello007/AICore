use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::priority::PolicyPriority,
    traits::process_controller::ProcessPrioritySnapshot,
};

#[cfg(target_os = "linux")]
use aicore_contracts::traits::process_controller::{AffinityMask, IProcessController};

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

#[cfg(target_os = "linux")]
impl IProcessController for LinuxProcessController {
    fn set_priority_captured(
        &self,
        state: &ProcessPrioritySnapshot,
        priority: PolicyPriority,
    ) -> ContractResult<()> {
        let (start, _) = decode_priority_snapshot(state)?;
        ensure_process_lifetime(start, read_process_start(state.pid)?)?;
        self.set_priority(state.pid, priority)?;
        ensure_process_lifetime(start, read_process_start(state.pid)?)
    }
    fn set_affinity_captured(
        &self,
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        let (start, _) = decode_priority_snapshot(state)?;
        ensure_process_lifetime(start, read_process_start(state.pid)?)?;
        self.set_affinity(state.pid, mask)?;
        ensure_process_lifetime(start, read_process_start(state.pid)?)?;
        let mut actual = self.get_affinity(state.pid)?.cores;
        actual.sort_unstable();
        let mut expected = mask.cores.clone();
        expected.sort_unstable();
        if actual != expected {
            return Err(ContractError::AdapterUnavailable(
                "affinity readback differs".into(),
            ));
        }
        Ok(())
    }
    fn restore_affinity(
        &self,
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        self.set_affinity_captured(state, mask)
    }
    fn set_priority(&self, pid: u32, priority: PolicyPriority) -> ContractResult<()> {
        let before = self.capture_priority(pid)?;
        let (start, current) = decode_priority_snapshot(&before)?;
        let desired = priority_to_nice(priority);
        validate_nice_change(current, desired)?;
        if current == desired {
            return Ok(());
        }
        ensure_process_lifetime(start, read_process_start(pid)?)?;
        set_native_nice(pid, desired)?;
        verify_priority_readback(
            &encode_priority_snapshot(pid, start, desired),
            &self.capture_priority(pid)?,
        )
    }

    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority> {
        Ok(nice_to_priority(read_native_nice(pid)?))
    }

    fn capture_priority(&self, pid: u32) -> ContractResult<ProcessPrioritySnapshot> {
        let start = read_process_start(pid)?;
        let nice = read_native_nice(pid)?;
        ensure_process_lifetime(start, read_process_start(pid)?)?;
        Ok(encode_priority_snapshot(pid, start, nice))
    }

    fn restore_priority(&self, state: &ProcessPrioritySnapshot) -> ContractResult<()> {
        let (start, nice) = decode_priority_snapshot(state)?;
        ensure_process_lifetime(start, read_process_start(state.pid)?)?;
        if read_native_nice(state.pid)? != nice {
            set_native_nice(state.pid, nice)?;
        }
        verify_priority_readback(state, &self.capture_priority(state.pid)?)
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

fn priority_to_nice(p: PolicyPriority) -> i32 {
    match p {
        PolicyPriority::Background => 19,
        PolicyPriority::Low => 10,
        PolicyPriority::Normal => 0,
        PolicyPriority::High => -5,
        PolicyPriority::Interactive => -10,
    }
}

fn nice_to_priority(nice: i32) -> PolicyPriority {
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

// Conservatively refuse demotion: unprivileged Linux callers cannot generally
// raise priority back. No speculative native mutation is used to probe authority.
pub(crate) fn validate_nice_change(current: i32, desired: i32) -> ContractResult<()> {
    if !(-20..=19).contains(&current) || !(-20..=19).contains(&desired) {
        return Err(ContractError::InvalidRequest(
            "native nice out of range".into(),
        ));
    }
    if desired > current {
        return Err(ContractError::PermissionDenied(
            "priority lowering refused: authority to restore the original nice is unverified"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn encode_priority_snapshot(pid: u32, start: u64, nice: i32) -> ProcessPrioritySnapshot {
    ProcessPrioritySnapshot {
        platform: "linux".into(),
        pid,
        state_id: format!("nice-v1:{pid}:{start}:{nice}"),
    }
}

pub(crate) fn decode_priority_snapshot(
    state: &ProcessPrioritySnapshot,
) -> ContractResult<(u64, i32)> {
    let invalid = || ContractError::ContractInvalid("invalid native priority snapshot".into());
    let parts: Vec<_> = state.state_id.split(':').collect();
    if state.platform != "linux"
        || parts.len() != 4
        || parts[0] != "nice-v1"
        || state.pid == 0
        || state.pid > i32::MAX as u32
    {
        return Err(invalid());
    }
    let pid = parts[1].parse::<u32>().map_err(|_| invalid())?;
    let start = parts[2].parse::<u64>().map_err(|_| invalid())?;
    let nice = parts[3].parse::<i32>().map_err(|_| invalid())?;
    if pid != state.pid || start == 0 || !(-20..=19).contains(&nice) {
        return Err(invalid());
    }
    Ok((start, nice))
}

pub(crate) fn ensure_process_lifetime(expected: u64, actual: u64) -> ContractResult<()> {
    if expected == 0 || expected != actual {
        return Err(ContractError::ResourceUnavailable(
            "process lifetime changed; refusing priority mutation".into(),
        ));
    }
    Ok(())
}

pub(crate) fn verify_priority_readback(
    expected: &ProcessPrioritySnapshot,
    actual: &ProcessPrioritySnapshot,
) -> ContractResult<()> {
    let (start, nice) = decode_priority_snapshot(expected)?;
    let (actual_start, actual_nice) = decode_priority_snapshot(actual)?;
    if expected.pid != actual.pid {
        return Err(ContractError::ResourceUnavailable(
            "priority readback PID differs".into(),
        ));
    }
    ensure_process_lifetime(start, actual_start)?;
    if nice != actual_nice {
        return Err(ContractError::AdapterUnavailable(
            "priority readback differs".into(),
        ));
    }
    Ok(())
}

pub(crate) fn parse_process_start(pid: u32, stat: &str) -> ContractResult<u64> {
    let invalid = || ContractError::ResourceUnavailable("invalid process identity".into());
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(invalid());
    }
    let (native_pid, _) = stat.split_once(' ').ok_or_else(invalid)?;
    if native_pid.parse::<u32>().ok() != Some(pid) {
        return Err(invalid());
    }
    // comm may contain spaces and ')'; starttime is field 22 (tail starts at 3).
    let (_, fields) = stat.rsplit_once(") ").ok_or_else(invalid)?;
    let start = fields
        .split_whitespace()
        .nth(19)
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(invalid)?;
    if start == 0 {
        return Err(invalid());
    }
    Ok(start)
}

#[cfg(target_os = "linux")]
fn read_process_start(pid: u32) -> ContractResult<u64> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(ContractError::InvalidRequest("invalid process ID".into()));
    }
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|e| ContractError::ResourceUnavailable(format!("read process identity: {e}")))?;
    parse_process_start(pid, &stat)
}

#[cfg(target_os = "linux")]
fn read_native_nice(pid: u32) -> ContractResult<i32> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(ContractError::InvalidRequest("invalid process ID".into()));
    }
    unsafe {
        *libc::__errno_location() = 0;
    }
    let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t) };
    let errno = std::io::Error::last_os_error();
    if errno.raw_os_error().unwrap_or(0) != 0 {
        return Err(ContractError::ResourceUnavailable(format!(
            "getpriority(pid={pid}) failed: {errno}"
        )));
    }
    Ok(nice)
}

#[cfg(target_os = "linux")]
fn set_native_nice(pid: u32, nice: i32) -> ContractResult<()> {
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };
    if rc == 0 {
        return Ok(());
    }
    let errno = std::io::Error::last_os_error();
    match errno.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => Err(ContractError::PermissionDenied(format!(
            "setpriority(pid={pid}, nice={nice}) denied: {errno}"
        ))),
        Some(libc::ESRCH) => Err(ContractError::ResourceUnavailable(format!(
            "no such process: pid={pid}"
        ))),
        _ => Err(ContractError::AdapterUnavailable(format!(
            "setpriority failed: {errno}"
        ))),
    }
}
