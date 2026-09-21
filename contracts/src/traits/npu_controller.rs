use crate::errors::ContractResult;

/// NPU 控制抽象。
pub trait INpuController: Send + Sync {
    fn submit_inference_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "npu control not implemented".to_string(),
        ))
    }

    fn get_status(&self) -> ContractResult<NpuControlStatus>;

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NpuControlStatus {
    pub controllable: bool,
    pub detail: Option<String>,
}
