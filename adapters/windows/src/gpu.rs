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
    fn status_for_detection(detected: bool) -> GpuControlStatus {
        GpuControlStatus {
            controllable: false,
            detail: Some(if detected {
                "NVIDIA SMI detected; reversible guarded control unavailable".to_string()
            } else {
                "no NVIDIA SMI detected; reversible guarded control unavailable".to_string()
            }),
        }
    }
    fn capabilities_for_detection(_detected: bool) -> Vec<&'static str> {
        vec!["gpu.status"]
    }
}
impl Default for WindowsGpuController {
    fn default() -> Self {
        Self::new()
    }
}
impl IGpuController for WindowsGpuController {
    fn get_status(&self) -> ContractResult<GpuControlStatus> {
        Ok(Self::status_for_detection(Self::nvidia_available()))
    }
    fn capabilities(&self) -> Vec<&'static str> {
        Self::capabilities_for_detection(false)
    }
    fn platform_name(&self) -> &'static str {
        "windows"
    }
    fn set_workload_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(ContractError::NotSupported(
            "Windows generic GPU workload hints require a reversible guarded actuator".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_detected_vendor_tool_does_not_grant_reversible_control() {
        for detected in [false, true] {
            let status = WindowsGpuController::status_for_detection(detected);
            assert!(
                !status.controllable,
                "detection cannot prove reversible control"
            );
            assert_eq!(
                WindowsGpuController::capabilities_for_detection(detected),
                vec!["gpu.status"]
            );
        }
    }
}
