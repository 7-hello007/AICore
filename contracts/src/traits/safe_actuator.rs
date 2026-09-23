use crate::{
    errors::ContractResult,
    hardware::capability::HardwareCapability,
    traits::hardware_executor::{HardwareExecutionRequest, HardwareExecutionResult},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActuationPermit {
    pub device_id: String,
    pub capability: HardwareCapability,
    pub expires_at_unix_ms: u64,
    pub model_only: bool,
}

pub trait ISafeHardwareActuator: Send + Sync {
    fn apply_authorized(
        &self,
        request: &HardwareExecutionRequest,
        permit: &ActuationPermit,
        now_unix_ms: u64,
    ) -> ContractResult<HardwareExecutionResult>;
}
