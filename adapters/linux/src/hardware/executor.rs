use aicore_contracts::{
    errors::ContractResult,
    traits::hardware_executor::{
        HardwareExecutionRequest, HardwareExecutionResult, HardwareExecutionStatus,
        IHardwareExecutor,
    },
};

/// Linux hardware executor (best effort).
///
/// Support path:
/// - AMD sysfs writes are unavailable until exact reversible state is supported.
///
/// Unsupported path: returns `NotSupported` without affecting Core.
pub struct LinuxHardwareExecutor;

impl LinuxHardwareExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxHardwareExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareExecutor for LinuxHardwareExecutor {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: req.device_id.clone(),
            status: HardwareExecutionStatus::NotSupported {
                reason: "Linux GPU control lacks exact original-state capture and independently verified restoration".into(),
            },
            latency_ms: 0,
        })
    }

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: device_id.into(),
            status: HardwareExecutionStatus::NotSupported {
                reason: "no captured original GPU state; auto is not restoration".into(),
            },
            latency_ms: 0,
        })
    }

    fn executor_name(&self) -> &'static str {
        "linux_hardware_executor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::traits::hardware_executor::HardwareExecutionHint;
    #[test]
    fn native_apply_and_release_never_invent_original_state() {
        let executor = LinuxHardwareExecutor::new();
        for id in ["drm:card0", "drm:../card0"] {
            let result = executor
                .apply(&HardwareExecutionRequest {
                    device_id: id.into(),
                    hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
                })
                .unwrap();
            assert!(matches!(
                result.status,
                HardwareExecutionStatus::NotSupported { .. }
            ));
            assert!(matches!(
                executor.release(id).unwrap().status,
                HardwareExecutionStatus::NotSupported { .. }
            ));
        }
    }
}
