use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::npu_controller::{INpuController, NpuControlStatus},
};
use std::path::Path;

const ACCEL_BASE: &str = "/sys/class/accel";

/// Linux NPU controller.
///
/// Currently Linux does not have NPU cross-vendor control ABI, thus exposing only the presence of equipment;
/// NPU inference routing is handled by the runtime or a vendor plugin.
pub struct LinuxNpuController;
impl LinuxNpuController {
    pub fn new() -> Self {
        Self
    }
}
impl Default for LinuxNpuController {
    fn default() -> Self {
        Self::new()
    }
}
impl INpuController for LinuxNpuController {
    fn submit_inference_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(ContractError::NotSupported(
            "Linux has no portable cross-vendor NPU control ABI".to_string(),
        ))
    }
    fn get_status(&self) -> ContractResult<NpuControlStatus> {
        Ok(NpuControlStatus {
            controllable: false,
            detail: Some(if Path::new(ACCEL_BASE).exists() {
                "NPU device classes detected; vendor runtime required for control".to_string()
            } else {
                "no NPU device class detected".to_string()
            }),
        })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["npu.status"]
    }
    fn platform_name(&self) -> &'static str {
        "linux"
    }
}
