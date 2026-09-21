use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::npu_controller::{INpuController, NpuControlStatus},
};

pub struct WindowsNpuController;
impl WindowsNpuController {
    pub fn new() -> Self {
        Self
    }
}
impl Default for WindowsNpuController {
    fn default() -> Self {
        Self::new()
    }
}
impl INpuController for WindowsNpuController {
    fn get_status(&self) -> ContractResult<NpuControlStatus> {
        Ok(NpuControlStatus { controllable: false, detail: Some("NPU control is vendor/runtime-specific; Windows ML or a vendor adapter is required".to_string()) })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["npu.status"]
    }
    fn platform_name(&self) -> &'static str {
        "windows"
    }
    fn submit_inference_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(ContractError::NotSupported(
            "Windows has no generic cross-vendor NPU control API".to_string(),
        ))
    }
}
