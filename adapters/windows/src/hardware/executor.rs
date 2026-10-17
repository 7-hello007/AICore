use aicore_contracts::{
    errors::ContractResult,
    traits::hardware_executor::{
        HardwareExecutionRequest, HardwareExecutionResult, HardwareExecutionStatus,
        IHardwareExecutor,
    },
};

/// Windows hardware implementer.
///
/// Universal GPU/NPU controls have no cross-vendor standard, so use capability-gated implementations:
/// - Native writes require exact reversible state and independent readback;
///   these are not provided by the current generic NVIDIA command path.
/// - Other devices: return `NotSupported` explicitly and do not forge execution success.
pub struct WindowsHardwareExecutor;
impl WindowsHardwareExecutor {
    pub fn new() -> Self {
        Self
    }
}
impl Default for WindowsHardwareExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareExecutor for WindowsHardwareExecutor {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: req.device_id.clone(),
            status: HardwareExecutionStatus::NotSupported {
                reason: "Windows vendor control lacks exact original-state capture, independent readback and restoration".into(),
            },
            latency_ms: 0,
        })
    }

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: device_id.to_string(),
            status: HardwareExecutionStatus::NotSupported {
                reason: "release is capability-specific; no safe universal restore value"
                    .to_string(),
            },
            latency_ms: 0,
        })
    }
    fn executor_name(&self) -> &'static str {
        "windows_hardware_executor"
    }
}

#[cfg(test)]
mod p0_tests {
    use super::*;
    use aicore_contracts::traits::hardware_executor::HardwareExecutionHint;
    #[test]
    fn p0_native_release_and_auto_refuse_missing_original_state() {
        let executor = WindowsHardwareExecutor::new();
        let result = executor
            .apply(&HardwareExecutionRequest {
                device_id: "nvidia:0".into(),
                hint: HardwareExecutionHint::AutoMode,
            })
            .unwrap();
        assert!(matches!(
            result.status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert!(matches!(
            executor.release("nvidia:0").unwrap().status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
    }
}
