use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::npu_controller::{INpuController, NpuControlStatus},
};
use std::path::Path;

const ACCEL_BASE: &str = "/sys/class/accel";

/// Linux NPU 控制器。
///
/// 当前 Linux 没有跨厂商通用 NPU 控制 ABI，因此只暴露设备存在状态；
/// NPU inference routing 由 Runtime / vendor plugin 负责。
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
