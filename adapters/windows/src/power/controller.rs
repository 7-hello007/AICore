use aicore_contracts::{
    errors::{ContractError, ContractResult},
    policy::resource_policy::PowerMode,
    traits::power_controller::{IPowerController, PowerStateSnapshot},
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

    // Windows-sys 0.61 will PowerGetActiveScheme/ PowerSetActiveScheme
    // Removed from Win32::System::Power, and therefore declared directly using external "system".
    // These two functions are exported by powrprof.dll and the signature is stable from Windows Vista.
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

    // Windows internal power source program GUID:
    //  - High performance: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
    //  - Balance: 381b4222-f694-41f0-9685-ff5b2df2e
    //  - Energy efficiency: a1841308-3541-4fab-bc81-f71556f20b4a
    const GUID_HIGH_PERF: [u8; 16] = [
        0xda, 0x7f, 0x5e, 0x8c, 0xbf, 0xe8, 0x96, 0x4a, 0x9a, 0x85, 0xa6, 0xe2, 0x3a, 0x8c, 0x63,
        0x5c,
    ];
    const GUID_BALANCED: [u8; 16] = [
        0x22, 0x42, 0x1b, 0x38, 0x94, 0xf6, 0xf0, 0x41, 0x96, 0x85, 0xff, 0x5b, 0xb2, 0x60, 0xdf,
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

    fn native_guid(bytes: [u8; 16]) -> windows_sys::core::GUID {
        windows_sys::core::GUID {
            data1: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            data2: u16::from_le_bytes([bytes[4], bytes[5]]),
            data3: u16::from_le_bytes([bytes[6], bytes[7]]),
            data4: [
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ],
        }
    }
    fn power_error(rc: u32, operation: &str) -> ContractError {
        let reason = format!("{operation}: Win32 error {rc}");
        match rc {
            5 => ContractError::PermissionDenied(reason),
            2 | 50 => ContractError::CapabilityUnavailable(reason),
            _ => ContractError::AdapterUnavailable(reason),
        }
    }
    fn active_guid() -> ContractResult<windows_sys::core::GUID> {
        let mut ptr = std::ptr::null_mut();
        // SAFETY: OS allocates the GUID; copy while valid, then LocalFree once.
        let rc = unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut ptr) };
        if rc != 0 || ptr.is_null() {
            if !ptr.is_null() {
                unsafe {
                    LocalFree(ptr.cast());
                }
            }
            return Err(power_error(rc, "PowerGetActiveScheme"));
        }
        let guid = unsafe { *ptr };
        unsafe {
            LocalFree(ptr.cast());
        }
        Ok(guid)
    }
    fn guid_string(g: &windows_sys::core::GUID) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            g.data1,
            g.data2,
            g.data3,
            g.data4[0],
            g.data4[1],
            g.data4[2],
            g.data4[3],
            g.data4[4],
            g.data4[5],
            g.data4[6],
            g.data4[7]
        )
    }
    fn parse_guid(s: &str) -> ContractResult<windows_sys::core::GUID> {
        let parts: Vec<_> = s.split('-').collect();
        let invalid = || ContractError::ContractInvalid("invalid power GUID snapshot".into());
        if parts.len() != 5
            || parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .any(|(p, n)| p.len() != n || !p.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid());
        }
        let tail = format!("{}{}", parts[3], parts[4]);
        let mut data4 = [0u8; 8];
        for (i, byte) in data4.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&tail[i * 2..i * 2 + 2], 16).map_err(|_| invalid())?;
        }
        Ok(windows_sys::core::GUID {
            data1: u32::from_str_radix(parts[0], 16).map_err(|_| invalid())?,
            data2: u16::from_str_radix(parts[1], 16).map_err(|_| invalid())?,
            data3: u16::from_str_radix(parts[2], 16).map_err(|_| invalid())?,
            data4,
        })
    }
    fn set_guid(guid: &windows_sys::core::GUID) -> ContractResult<()> {
        // SAFETY: an aligned native GUID is valid for this synchronous OS call.
        let rc = unsafe { PowerSetActiveScheme(std::ptr::null_mut(), guid) };
        if rc != 0 {
            return Err(power_error(rc, "PowerSetActiveScheme"));
        }
        if guid_string(&active_guid()?) != guid_string(guid) {
            return Err(ContractError::AdapterUnavailable(
                "power scheme readback mismatch".into(),
            ));
        }
        Ok(())
    }
    pub fn capture_state_impl() -> ContractResult<PowerStateSnapshot> {
        Ok(PowerStateSnapshot {
            platform: "windows".into(),
            state_id: guid_string(&active_guid()?),
        })
    }
    pub fn restore_state_impl(state: &PowerStateSnapshot) -> ContractResult<()> {
        if state.platform != "windows" {
            return Err(ContractError::ContractInvalid(
                "power snapshot adapter mismatch".into(),
            ));
        }
        set_guid(&parse_guid(&state.state_id)?)
    }
    pub fn set_profile_impl(mode: PowerMode) -> ContractResult<PowerMode> {
        set_guid(&native_guid(guid_for(mode)))?;
        Ok(mode)
    }
    pub fn get_profile_impl() -> ContractResult<PowerMode> {
        let current = guid_string(&active_guid()?);
        for mode in [
            PowerMode::Balanced,
            PowerMode::Performance,
            PowerMode::PowerSave,
        ] {
            if current == guid_string(&native_guid(guid_for(mode))) {
                return Ok(mode);
            }
        }
        Err(ContractError::CapabilityUnavailable(
            "active Windows power scheme is not a built-in profile".into(),
        ))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn balanced_guid_matches_documented_profile_and_snapshot_roundtrip() {
            let guid = native_guid(GUID_BALANCED);
            assert_eq!(guid_string(&guid), "381b4222-f694-41f0-9685-ff5bb260df2e");
            assert_eq!(
                guid_string(&parse_guid(&guid_string(&guid)).unwrap()),
                guid_string(&guid)
            );
            assert!(parse_guid("bad-guid").is_err());
            assert!(matches!(
                power_error(5, "test"),
                ContractError::PermissionDenied(_)
            ));
        }
    }
}

impl IPowerController for WindowsPowerController {
    fn capture_state(&self) -> ContractResult<PowerStateSnapshot> {
        #[cfg(target_os = "windows")]
        {
            imp::capture_state_impl()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows power snapshot unavailable".into(),
            ))
        }
    }
    fn restore_state(&self, _state: &PowerStateSnapshot) -> ContractResult<()> {
        #[cfg(target_os = "windows")]
        {
            imp::restore_state_impl(_state)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ContractError::NotSupported(
                "Windows power restore unavailable".into(),
            ))
        }
    }
    fn set_profile(&self, _mode: PowerMode) -> ContractResult<PowerMode> {
        #[cfg(target_os = "windows")]
        {
            imp::set_profile_impl(_mode)
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
            imp::get_profile_impl()
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
