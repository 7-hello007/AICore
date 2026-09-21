use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::resource_policy::PowerMode,
    traits::power_controller::IPowerController,
};

pub struct WindowsPowerController;

impl WindowsPowerController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsPowerController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::LocalFree;

    // windows-sys 0.61 将 PowerGetActiveScheme / PowerSetActiveScheme
    // 从 Win32::System::Power 移除，因此直接用 extern "system" 声明。
    // 这两个函数由 powrprof.dll 导出，签名自 Windows Vista 起稳定。
    #[link(name = "powrprof")]
    extern "system" {
        fn PowerGetActiveScheme(
            user_root_power_key: *mut core::ffi::c_void,
            active_policy_guid: *mut *mut windows_sys::core::GUID,
        ) -> u32;

        fn PowerSetActiveScheme(
            user_root_power_key: *mut core::ffi::c_void,
            scheme_guid: *const windows_sys::core::GUID,
        ) -> u32;
    }

    // Windows 内置电源方案 GUID：
    //  - 高性能: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
    //  - 平衡:   381b4222-f694-41f0-9685-ff5bb260df2e
    //  - 节能:   a1841308-3541-4fab-bc81-f71556f20b4a
    const GUID_HIGH_PERF: [u8; 16] = [
        0xda, 0x7f, 0x5e, 0x8c, 0xbf, 0xe8, 0x96, 0x4a, 0x9a, 0x85, 0xa6, 0xe2, 0x3a, 0x8c, 0x63,
        0x5c,
    ];
    const GUID_BALANCED: [u8; 16] = [
        0x22, 0x42, 0x1b, 0x38, 0xf4, 0x69, 0xf0, 0x41, 0x96, 0x85, 0xff, 0x5b, 0xb2, 0x60, 0xdf,
        0x2e,
    ];
    const GUID_POWER_SAVER: [u8; 16] = [
        0x08, 0x13, 0x84, 0xa1, 0x41, 0x35, 0xab, 0x4f, 0xbc, 0x81, 0xf7, 0x15, 0x56, 0xf2, 0x0b,
        0x4a,
    ];

    fn guid_for(mode: PowerMode) -> [u8; 16] {
        match mode {
            PowerMode::Performance => GUID_HIGH_PERF,
            PowerMode::Balanced => GUID_BALANCED,
            PowerMode::PowerSave => GUID_POWER_SAVER,
        }
    }

    pub fn set_profile_impl(mode: PowerMode) -> ContractResult<PowerMode> {
        let guid = guid_for(mode);
        unsafe {
            let rc = PowerSetActiveScheme(
                std::ptr::null_mut(),
                guid.as_ptr() as *const windows_sys::core::GUID,
            );
            if rc != 0 {
                let e = std::io::Error::last_os_error();
                return Err(match e.raw_os_error() {
                    Some(5) => ContractError::PermissionDenied(format!(
                        "PowerSetActiveScheme denied: {}",
                        e
                    )),
                    _ => ContractError::AdapterUnavailable(format!(
                        "PowerSetActiveScheme failed: {}",
                        e
                    )),
                });
            }
        }
        Ok(mode)
    }

    pub fn get_profile_impl() -> ContractResult<PowerMode> {
        unsafe {
            let mut guid_ptr: *mut windows_sys::core::GUID = std::ptr::null_mut();
            let rc = PowerGetActiveScheme(std::ptr::null_mut(), &mut guid_ptr);
            if rc != 0 || guid_ptr.is_null() {
                return Err(ContractError::AdapterUnavailable(
                    "PowerGetActiveScheme failed".to_string(),
                ));
            }
            let bytes = std::slice::from_raw_parts(guid_ptr as *const u8, 16);
            let mode = if bytes == GUID_HIGH_PERF {
                PowerMode::Performance
            } else if bytes == GUID_POWER_SAVER {
                PowerMode::PowerSave
            } else {
                PowerMode::Balanced
            };
            LocalFree(guid_ptr as *mut core::ffi::c_void);
            Ok(mode)
        }
    }
}

impl IPowerController for WindowsPowerController {
    fn set_profile(&self, _mode: PowerMode) -> ContractResult<PowerMode> {
        #[cfg(target_os = "windows")]
        {
            return imp::set_profile_impl(_mode);
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsPowerController only available on Windows".to_string(),
            ))
        }
    }

    fn get_profile(&self) -> ContractResult<PowerMode> {
        #[cfg(target_os = "windows")]
        {
            return imp::get_profile_impl();
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "WindowsPowerController only available on Windows".to_string(),
            ))
        }
    }

    fn capabilities(&self) -> Vec<&'static str> {
        if cfg!(target_os = "windows") {
            vec!["power.profile.read", "power.profile.write"]
        } else {
            Vec::new()
        }
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }
}
