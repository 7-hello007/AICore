use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::priority::PolicyPriority,
    traits::process_controller::{AffinityMask, IProcessController, ProcessPrioritySnapshot},
};

pub struct WindowsProcessController;

impl WindowsProcessController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsProcessController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;

    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetPriorityClass, GetProcessTimes, OpenProcess, SetPriorityClass,
        ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
        IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SET_INFORMATION, REALTIME_PRIORITY_CLASS,
    };

    pub fn set_priority_impl(pid: u32, priority: PolicyPriority) -> ContractResult<()> {
        let state = capture_priority_impl(pid)?;
        set_priority_captured_impl(&state, priority)
    }

    pub fn set_priority_captured_impl(
        state: &ProcessPrioritySnapshot,
        priority: PolicyPriority,
    ) -> ContractResult<()> {
        let class = priority_to_class(priority);
        with_captured_process(state, |handle| {
            let original = unsafe { GetPriorityClass(handle) };
            if original == 0 {
                return Err(last_error("GetPriorityClass"));
            }
            validate_reversible_priority(original, class)?;
            if original != class && unsafe { SetPriorityClass(handle, class) } == 0 {
                return Err(map_error(std::io::Error::last_os_error(), state.pid));
            }
            if unsafe { GetPriorityClass(handle) } != class {
                return Err(ContractError::AdapterUnavailable(
                    "priority apply readback differs".into(),
                ));
            }
            Ok(())
        })
    }

    pub(super) fn validate_reversible_priority(original: u32, desired: u32) -> ContractResult<()> {
        if original == REALTIME_PRIORITY_CLASS && desired != original {
            return Err(ContractError::NotSupported(
                "realtime demotion refused: exact restoration privilege is not guaranteed".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn with_captured_process<T>(
        state: &ProcessPrioritySnapshot,
        action: impl FnOnce(windows_sys::Win32::Foundation::HANDLE) -> ContractResult<T>,
    ) -> ContractResult<T> {
        let invalid = || ContractError::ContractInvalid("invalid native process snapshot".into());
        if state.platform != "windows" {
            return Err(invalid());
        }
        let (class, created) = state
            .state_id
            .strip_prefix("class=")
            .and_then(|s| s.split_once(":created="))
            .ok_or_else(invalid)?;
        let _: u32 = class.parse().map_err(|_| invalid())?;
        let created: u64 = created.parse().map_err(|_| invalid())?;
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION
                    | windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION
                    | PROCESS_SET_INFORMATION,
                0,
                state.pid,
            )
        };
        if handle.is_null() {
            return Err(last_error("OpenProcess (captured mutation)"));
        }
        let result = (|| {
            if lifetime(handle)? != created {
                return Err(ContractError::PermissionDenied(
                    "process lifetime changed since capture".into(),
                ));
            }
            action(handle)
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }

    pub fn get_priority_impl(pid: u32) -> ContractResult<PolicyPriority> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(last_error("OpenProcess (query)"));
            }
            let class = GetPriorityClass(handle);
            CloseHandle(handle);
            if class == 0 {
                return Err(last_error("GetPriorityClass"));
            }
            Ok(class_to_priority(class))
        }
    }

    fn lifetime(handle: windows_sys::Win32::Foundation::HANDLE) -> ContractResult<u64> {
        let mut created = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exited = created;
        let mut kernel = created;
        let mut user = created;
        // SAFETY: valid owned process handle and writable stack FILETIME values.
        if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
            == 0
        {
            return Err(last_error("GetProcessTimes"));
        }
        Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    pub fn capture_priority_impl(pid: u32) -> ContractResult<ProcessPrioritySnapshot> {
        // SAFETY: queried handle is closed exactly once after all synchronous reads.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return Err(last_error("OpenProcess (capture priority)"));
        }
        let result = (|| {
            let class = unsafe { GetPriorityClass(handle) };
            if class == 0 {
                return Err(last_error("GetPriorityClass"));
            }
            Ok(ProcessPrioritySnapshot {
                platform: "windows".into(),
                pid,
                state_id: format!("class={class}:created={}", lifetime(handle)?),
            })
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }

    pub fn restore_priority_impl(state: &ProcessPrioritySnapshot) -> ContractResult<()> {
        if state.platform != "windows" {
            return Err(ContractError::ContractInvalid(
                "priority snapshot adapter mismatch".into(),
            ));
        }
        let invalid = || ContractError::ContractInvalid("invalid native priority snapshot".into());
        let (class, created) = state
            .state_id
            .strip_prefix("class=")
            .and_then(|s| s.split_once(":created="))
            .ok_or_else(invalid)?;
        let class: u32 = class.parse().map_err(|_| invalid())?;
        let created: u64 = created.parse().map_err(|_| invalid())?;
        if ![
            IDLE_PRIORITY_CLASS,
            BELOW_NORMAL_PRIORITY_CLASS,
            NORMAL_PRIORITY_CLASS,
            ABOVE_NORMAL_PRIORITY_CLASS,
            HIGH_PRIORITY_CLASS,
            REALTIME_PRIORITY_CLASS,
        ]
        .contains(&class)
        {
            return Err(invalid());
        }
        // Bind identity check and write to the same handle; a reused PID is never restored.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION,
                0,
                state.pid,
            )
        };
        if handle.is_null() {
            return Err(last_error("OpenProcess (restore priority)"));
        }
        let result = (|| {
            if lifetime(handle)? != created {
                return Err(ContractError::PermissionDenied(
                    "process lifetime changed since capture".into(),
                ));
            }
            if unsafe { GetPriorityClass(handle) } != class
                && unsafe { SetPriorityClass(handle, class) } == 0
            {
                return Err(last_error("SetPriorityClass (restore)"));
            }
            if unsafe { GetPriorityClass(handle) } != class {
                return Err(ContractError::AdapterUnavailable(
                    "native priority restoration readback differs".into(),
                ));
            }
            Ok(())
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }

    fn priority_to_class(p: PolicyPriority) -> u32 {
        match p {
            PolicyPriority::Background => IDLE_PRIORITY_CLASS,
            PolicyPriority::Low => BELOW_NORMAL_PRIORITY_CLASS,
            PolicyPriority::Normal => NORMAL_PRIORITY_CLASS,
            PolicyPriority::High => ABOVE_NORMAL_PRIORITY_CLASS,
            PolicyPriority::Interactive => HIGH_PRIORITY_CLASS,
        }
    }

    fn class_to_priority(c: u32) -> PolicyPriority {
        match c {
            IDLE_PRIORITY_CLASS => PolicyPriority::Background,
            BELOW_NORMAL_PRIORITY_CLASS => PolicyPriority::Low,
            NORMAL_PRIORITY_CLASS => PolicyPriority::Normal,
            ABOVE_NORMAL_PRIORITY_CLASS => PolicyPriority::High,
            HIGH_PRIORITY_CLASS | REALTIME_PRIORITY_CLASS => PolicyPriority::Interactive,
            _ => PolicyPriority::Normal,
        }
    }

    fn last_error(ctx: &str) -> ContractError {
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(5) => ContractError::PermissionDenied(format!("{}: {}", ctx, e)),
            Some(87) => ContractError::InvalidRequest(format!("{}: {}", ctx, e)),
            _ => ContractError::AdapterUnavailable(format!("{}: {}", ctx, e)),
        }
    }

    fn map_error(e: std::io::Error, pid: u32) -> ContractError {
        match e.raw_os_error() {
            Some(5) => ContractError::PermissionDenied(format!("set priority pid={}: {}", pid, e)),
            Some(87) => ContractError::InvalidRequest(format!("set priority pid={}: {}", pid, e)),
            _ => ContractError::AdapterUnavailable(format!("set priority pid={}: {}", pid, e)),
        }
    }
}

#[cfg(target_os = "windows")]
mod affinity_imp {
    use super::*;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetProcessAffinityMask, OpenProcess, SetProcessAffinityMask, PROCESS_QUERY_INFORMATION,
        PROCESS_SET_INFORMATION,
    };

    pub fn set_captured(
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        if mask.is_empty() || mask.cores.iter().any(|core| *core >= usize::BITS) {
            return Err(ContractError::InvalidRequest(
                "invalid affinity mask".into(),
            ));
        }
        let bits = mask
            .cores
            .iter()
            .fold(0usize, |bits, core| bits | (1usize << core));
        super::imp::with_captured_process(state, |handle| {
            if unsafe { SetProcessAffinityMask(handle, bits) } == 0 {
                return Err(last_affinity_error("SetProcessAffinityMask (captured)"));
            }
            let mut actual = 0usize;
            let mut system = 0usize;
            if unsafe { GetProcessAffinityMask(handle, &mut actual, &mut system) } == 0 {
                return Err(last_affinity_error("GetProcessAffinityMask (captured)"));
            }
            if actual != bits {
                return Err(ContractError::AdapterUnavailable(
                    "affinity readback differs".into(),
                ));
            }
            Ok(())
        })
    }

    pub fn set_affinity_impl(pid: u32, mask: &AffinityMask) -> ContractResult<()> {
        if mask.is_empty() {
            return Err(ContractError::InvalidRequest(
                "affinity mask is empty".to_string(),
            ));
        }

        let mut mask_bits: usize = 0;
        for &core in &mask.cores {
            if core >= 64 {
                return Err(ContractError::InvalidRequest(format!(
                    "core {} out of range for 64-bit affinity mask",
                    core
                )));
            }
            mask_bits |= 1usize << core;
        }

        unsafe {
            let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(last_affinity_error("OpenProcess (set affinity)"));
            }
            let ok = SetProcessAffinityMask(handle, mask_bits);
            let err = if ok == 0 {
                Some(std::io::Error::last_os_error())
            } else {
                None
            };
            CloseHandle(handle);
            match err {
                Some(e) => Err(map_affinity_error(e, pid)),
                None => Ok(()),
            }
        }
    }

    pub fn get_affinity_impl(pid: u32) -> ContractResult<AffinityMask> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(last_affinity_error("OpenProcess (get affinity)"));
            }
            let mut process_mask: usize = 0;
            let mut system_mask: usize = 0;
            let ok = GetProcessAffinityMask(handle, &mut process_mask, &mut system_mask);
            CloseHandle(handle);
            if ok == 0 {
                return Err(last_affinity_error("GetProcessAffinityMask"));
            }
            let mut cores = Vec::new();
            for i in 0..64u32 {
                if (process_mask >> i) & 1 == 1 {
                    cores.push(i);
                }
            }
            Ok(AffinityMask::new(cores))
        }
    }

    fn last_affinity_error(ctx: &str) -> ContractError {
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(5) => ContractError::PermissionDenied(format!("{}: {}", ctx, e)),
            Some(87) => ContractError::InvalidRequest(format!("{}: {}", ctx, e)),
            _ => ContractError::AdapterUnavailable(format!("{}: {}", ctx, e)),
        }
    }

    fn map_affinity_error(e: std::io::Error, pid: u32) -> ContractError {
        match e.raw_os_error() {
            Some(5) => ContractError::PermissionDenied(format!("set affinity pid={}: {}", pid, e)),
            Some(87) => ContractError::InvalidRequest(format!("set affinity pid={}: {}", pid, e)),
            _ => ContractError::AdapterUnavailable(format!("set affinity pid={}: {}", pid, e)),
        }
    }
}

impl IProcessController for WindowsProcessController {
    fn set_priority_captured(
        &self,
        _state: &ProcessPrioritySnapshot,
        _priority: PolicyPriority,
    ) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            imp::set_priority_captured_impl(_state, _priority)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows process mutation unavailable".into(),
            ))
        }
    }
    fn set_affinity_captured(
        &self,
        _state: &ProcessPrioritySnapshot,
        _mask: &AffinityMask,
    ) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            affinity_imp::set_captured(_state, _mask)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows process mutation unavailable".into(),
            ))
        }
    }
    fn restore_affinity(
        &self,
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        self.set_affinity_captured(state, mask)
    }
    fn capture_priority(&self, _pid: u32) -> ContractResult<ProcessPrioritySnapshot> {
        #[cfg(target_os = "windows")]
        {
            imp::capture_priority_impl(_pid)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows priority capture unavailable".into(),
            ))
        }
    }
    fn restore_priority(&self, _state: &ProcessPrioritySnapshot) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            imp::restore_priority_impl(_state)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows priority restoration unavailable".into(),
            ))
        }
    }
    fn set_priority(&self, _pid: u32, _priority: PolicyPriority) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            imp::set_priority_impl(_pid, _priority)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsProcessController only available on Windows".to_string(),
            ))
        }
    }

    fn get_priority(&self, _pid: u32) -> ContractResult<PolicyPriority> {
        #[cfg(target_os = "windows")]
        {
            imp::get_priority_impl(_pid)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsProcessController only available on Windows".to_string(),
            ))
        }
    }

    fn set_affinity(&self, _pid: u32, _mask: &AffinityMask) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            affinity_imp::set_affinity_impl(_pid, _mask)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsProcessController only available on Windows".to_string(),
            ))
        }
    }

    fn get_affinity(&self, _pid: u32) -> ContractResult<AffinityMask> {
        #[cfg(target_os = "windows")]
        {
            affinity_imp::get_affinity_impl(_pid)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsProcessController only available on Windows".to_string(),
            ))
        }
    }

    fn capabilities(&self) -> Vec<&'static str> {
        if cfg!(target_os = "windows") {
            vec![
                "process.priority.read",
                "process.priority.write",
                "process.affinity.read",
                "process.affinity.write",
            ]
        } else {
            Vec::new()
        }
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }
}

#[cfg(all(test, target_os = "windows"))]
mod priority_snapshot_tests {
    use super::*;
    #[test]
    fn p0_native_priority_snapshot_retains_class_and_process_lifetime() {
        // Read-only native evidence: no priority setter or child installation.
        let snapshot = WindowsProcessController::new()
            .capture_priority(std::process::id())
            .unwrap();
        assert!(
            snapshot.state_id.starts_with("class="),
            "native class must not be replaced by a PolicyPriority bucket: {}",
            snapshot.state_id
        );
        assert!(
            snapshot.state_id.contains(":created="),
            "snapshot must bind the process lifetime"
        );
    }
    #[test]
    fn p0_affinity_restore_refuses_changed_process_lifetime() {
        let controller = WindowsProcessController::new();
        let pid = std::process::id();
        let original = controller.get_affinity(pid).unwrap();
        let mut snapshot = controller.capture_priority(pid).unwrap();
        snapshot.state_id = snapshot
            .state_id
            .split_once(":created=")
            .unwrap()
            .0
            .to_string()
            + ":created=0";
        // Even the RED path requests the current mask, never a different affinity.
        assert!(matches!(
            controller.restore_affinity(&snapshot, &original),
            Err(ContractError::PermissionDenied(_))
        ));
        assert_eq!(controller.get_affinity(pid).unwrap(), original);
    }
    #[test]
    fn p0_apply_refuses_changed_process_lifetime() {
        let controller = WindowsProcessController::new();
        let pid = std::process::id();
        let mut snapshot = controller.capture_priority(pid).unwrap();
        snapshot.state_id = snapshot
            .state_id
            .split_once(":created=")
            .unwrap()
            .0
            .to_string()
            + ":created=0";
        let priority = controller.get_priority(pid).unwrap();
        let affinity = controller.get_affinity(pid).unwrap();
        assert!(controller
            .set_priority_captured(&snapshot, priority)
            .is_err());
        assert!(controller
            .set_affinity_captured(&snapshot, &affinity)
            .is_err());
        assert_eq!(controller.get_priority(pid).unwrap(), priority);
        assert_eq!(controller.get_affinity(pid).unwrap(), affinity);
    }
    #[test]
    fn p0_realtime_demotion_requires_guaranteed_reverse_authority() {
        use windows_sys::Win32::System::Threading::{
            NORMAL_PRIORITY_CLASS, REALTIME_PRIORITY_CLASS,
        };
        // Pure validation seam: never changes a real process to REALTIME.
        assert!(
            imp::validate_reversible_priority(REALTIME_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS)
                .is_err()
        );
        assert!(
            imp::validate_reversible_priority(NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS).is_ok()
        );
    }
}
