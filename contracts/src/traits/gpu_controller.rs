use crate::errors::ContractResult;

/// GPU controls abstraction.
///
/// All platforms in phase V2-D return CapabilityUnavailable;
/// Vendor-runtime access remains a V3 implementation concern.
pub trait IGpuController: Send + Sync {
    fn set_workload_hint(&self, _hint: &str) -> ContractResult<()> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "gpu control not implemented".to_string(),
        ))
    }

    fn get_status(&self) -> ContractResult<GpuControlStatus>;

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuControlStatus {
    pub controllable: bool,
    pub detail: Option<String>,
}
