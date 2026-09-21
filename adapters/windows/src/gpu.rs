use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::gpu_controller::{GpuControlStatus, IGpuController},
};
use std::process::Command;

pub struct WindowsGpuController;
impl WindowsGpuController {
    pub fn new() -> Self {
        Self
    }
    fn nvidia_available() -> bool {
        Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
impl Default for WindowsGpuController {
    fn default() -> Self {
        Self::new()
    }
}
impl IGpuController for WindowsGpuController {
    fn get_status(&self) -> ContractResult<GpuControlStatus> {
        Ok(GpuControlStatus {
            controllable: Self::nvidia_available(),
            detail: Some(if Self::nvidia_available() {
                "NVIDIA SMI detected; power controls available through hardware executor"
                    .to_string()
            } else {
                "no portable generic GPU control path; vendor adapter required".to_string()
            }),
        })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        if Self::nvidia_available() {
            vec!["gpu.status", "gpu.nvidia.power_limit"]
        } else {
            vec!["gpu.status"]
        }
    }
    fn platform_name(&self) -> &'static str {
        "windows"
    }
    fn set_workload_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(ContractError::NotSupported(
            "Windows generic GPU workload hints are vendor-specific; use IHardwareExecutor/plugin"
                .to_string(),
        ))
    }
}
