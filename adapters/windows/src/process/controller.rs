use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::priority::PolicyPriority,
    traits::process_controller::{AffinityMask, IProcessController},
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

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetPriorityClass, OpenProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
        BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS,
        NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
        REALTIME_PRIORITY_CLASS,
    };

    pub fn set_priority_impl(pid: u32, priority: PolicyPriority) -> ContractResult<()> {
        let class = priority_to_class(priority);
        unsafe {
            let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(last_error("OpenProcess (set)"));
            }
            let ok = SetPriorityClass(handle, class);
            let err = if ok == 0 {
                Some(std::io::Error::last_os_error())
            } else {
                None
            };
            CloseHandle(handle);
            match err {
                Some(e) => Err(map_error(e, pid)),
                None => Ok(()),
            }
        }
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
    fn set_priority(&self, _pid: u32, _priority: PolicyPriority) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            return imp::set_priority_impl(_pid, _priority);
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
            return imp::get_priority_impl(_pid);
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
            return affinity_imp::set_affinity_impl(_pid, _mask);
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
            return affinity_imp::get_affinity_impl(_pid);
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
