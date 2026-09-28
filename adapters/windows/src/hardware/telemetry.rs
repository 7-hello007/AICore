use aicore_contracts::{
    errors::{ContractError, ContractResult},
    hardware::telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample},
    traits::hardware_telemetry::IHardwareTelemetryProvider,
};
use std::process::Command;

pub struct WindowsHardwareTelemetryProvider;
impl WindowsHardwareTelemetryProvider {
    pub fn new() -> Self {
        Self
    }
    fn query(args: &[&str]) -> Option<String> {
        Command::new("nvidia-smi")
            .args(args)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
    }
}
impl Default for WindowsHardwareTelemetryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareTelemetryProvider for WindowsHardwareTelemetryProvider {
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo> {
        let mut devices = dxgi_devices();
        devices.extend(self.nvidia_devices());
        devices
    }
    fn npu_devices(&self) -> Vec<HardwareDeviceInfo> {
        compute_accelerator_devices()
    }
    fn sample_gpu(&self, device_id: &str) -> ContractResult<GpuTelemetrySample> {
        if device_id.starts_with("gpu:dxgi:") {
            if !dxgi_devices().iter().any(|d| d.id == device_id) {
                return Err(ContractError::InvalidRequest(
                    "unknown DXGI device id".into(),
                ));
            }
            return Err(ContractError::NotSupported(
                "DXGI identity discovery does not supply utilization or temperature telemetry"
                    .into(),
            ));
        }
        self.sample_nvidia(device_id)
    }
    fn sample_npu(&self, _device_id: &str) -> ContractResult<NpuTelemetrySample> {
        Err(ContractError::NotSupported(
            "Windows NPU telemetry requires a live vendor provider".into(),
        ))
    }
    fn provider_name(&self) -> &'static str {
        "windows_hardware_telemetry"
    }
}
impl WindowsHardwareTelemetryProvider {
    fn nvidia_devices(&self) -> Vec<HardwareDeviceInfo> {
        let Some(out) = Self::query(&[
            "--query-gpu=index,name,driver_version",
            "--format=csv,noheader,nounits",
        ]) else {
            return Vec::new();
        };
        out.lines()
            .filter_map(|line| {
                let parts: Vec<_> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() < 3 {
                    return None;
                }
                Some(HardwareDeviceInfo {
                    id: format!("nvidia:{}", parts[0]),
                    vendor: "NVIDIA".to_string(),
                    name: parts[1].to_string(),
                    driver: Some(parts[2].to_string()),
                })
            })
            .collect()
    }
    fn sample_nvidia(&self, device_id: &str) -> ContractResult<GpuTelemetrySample> {
        let index = device_id.strip_prefix("nvidia:").ok_or_else(|| {
            ContractError::InvalidRequest(format!("unsupported gpu id: {}", device_id))
        })?;
        let out = Self::query(&["-i", index, "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,name,driver_version", "--format=csv,noheader,nounits"])
            .ok_or_else(|| ContractError::NotSupported("nvidia-smi unavailable or query failed".to_string()))?;
        let p: Vec<_> = out.split(',').map(|s| s.trim()).collect();
        if p.len() < 6 {
            return Err(ContractError::AdapterUnavailable(
                "unexpected nvidia-smi output".to_string(),
            ));
        }
        let parse_f32 = |s: &str| s.parse::<f32>().ok();
        let parse_u64 = |s: &str| s.parse::<u64>().ok();
        Ok(GpuTelemetrySample {
            device: HardwareDeviceInfo {
                id: device_id.to_string(),
                vendor: "NVIDIA".to_string(),
                name: p[4].to_string(),
                driver: Some(p[5].to_string()),
            },
            utilization_percent: parse_f32(p[0]),
            memory_used_mb: parse_u64(p[1]),
            memory_total_mb: parse_u64(p[2]),
            temperature_celsius: parse_f32(p[3]),
            sampled_at_unix_ms: now_unix_ms(),
        })
    }
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn dxgi_devices() -> Vec<HardwareDeviceInfo> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_DESC1};
    // SAFETY: the returned COM factory owns its references; descriptor buffers
    // remain writable throughout query-only calls. No device state is modified.
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for index in 0..64 {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let mut desc = DXGI_ADAPTER_DESC1::default();
        if unsafe { adapter.GetDesc1(&mut desc) }.is_err() || desc.Flags & 2 != 0 {
            continue;
        }
        let length = desc
            .Description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(desc.Description.len());
        let vendor = match desc.VendorId {
            0x1002 => "AMD".into(),
            0x10de => "NVIDIA".into(),
            0x8086 => "Intel".into(),
            other => format!("PCI:{other:04x}"),
        };
        devices.push(HardwareDeviceInfo {
            id: format!("gpu:dxgi:{index}"),
            vendor,
            name: String::from_utf16_lossy(&desc.Description[..length]),
            driver: None,
        });
    }
    devices
}
#[cfg(not(target_os = "windows"))]
fn dxgi_devices() -> Vec<HardwareDeviceInfo> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn compute_accelerator_devices() -> Vec<HardwareDeviceInfo> {
    use windows::{
        core::{GUID, PCWSTR},
        Win32::{Devices::DeviceAndDriverInstallation::*, Foundation::HWND},
    };
    let class: Vec<u16> = "ComputeAccelerator".encode_utf16().chain(Some(0)).collect();
    let mut guids = [GUID::zeroed(); 32];
    let mut count = 0;
    // SAFETY: query-only SetupAPI calls receive live, sized output buffers.
    if unsafe { SetupDiClassGuidsFromNameW(PCWSTR(class.as_ptr()), &mut guids, &mut count) }
        .is_err()
        || count > 32
    {
        return Vec::new();
    }
    fn property(set: HDEVINFO, info: &SP_DEVINFO_DATA, kind: u32) -> Option<String> {
        let mut bytes = [0u8; 4096];
        // SAFETY: the device info set and descriptor remain alive in the caller.
        if unsafe {
            SetupDiGetDeviceRegistryPropertyW(set, info, kind, None, Some(&mut bytes), None)
        }
        .is_err()
        {
            return None;
        }
        let text: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|v| *v != 0)
            .collect();
        if text.is_empty() {
            None
        } else {
            Some(String::from_utf16_lossy(&text))
        }
    }
    struct InfoSet(HDEVINFO);
    impl Drop for InfoSet {
        fn drop(&mut self) {
            // SAFETY: this guard owns the query-only SetupAPI set.
            let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
        }
    }
    let mut devices = Vec::new();
    for guid in &guids[..count as usize] {
        // SAFETY: valid class GUID; no window and no modifying flags.
        let Ok(set) =
            (unsafe { SetupDiGetClassDevsW(Some(guid), PCWSTR::null(), HWND(0), DIGCF_PRESENT) })
        else {
            continue;
        };
        let set = InfoSet(set);
        for index in 0..1024 {
            let mut info = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            // SAFETY: the descriptor is sized and writable.
            if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) }.is_err() {
                break;
            }
            let mut identity = [0u16; 1024];
            // SAFETY: valid descriptor and bounded UTF-16 identity buffer.
            if unsafe { SetupDiGetDeviceInstanceIdW(set.0, &info, Some(&mut identity), None) }
                .is_err()
            {
                continue;
            }
            let size = identity
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(identity.len());
            let id = String::from_utf16_lossy(&identity[..size]);
            let Some(name) = property(set.0, &info, SPDRP_FRIENDLYNAME)
                .or_else(|| property(set.0, &info, SPDRP_DEVICEDESC))
            else {
                continue;
            };
            let vendor = property(set.0, &info, SPDRP_MFG).unwrap_or_else(|| "Unavailable".into());
            devices.push(HardwareDeviceInfo {
                id: format!("npu:pnp:{id}"),
                vendor,
                name,
                driver: None,
            });
        }
    }
    devices
}
#[cfg(not(target_os = "windows"))]
fn compute_accelerator_devices() -> Vec<HardwareDeviceInfo> {
    Vec::new()
}
