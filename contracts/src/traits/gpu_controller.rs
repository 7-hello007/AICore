use crate::errors::ContractResult;

/// GPU 控制抽象。
///
/// V2-D 阶段所有平台返回 CapabilityUnavailable；
/// 真实实现留给 V3 的 vendor runtime 接入。
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
