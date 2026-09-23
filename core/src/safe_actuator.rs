use std::sync::Arc;

use aicore_contracts::{
    errors::ContractResult,
    hardware::{capability::HardwareCapability, matrix::DeviceCapabilityMatrix},
    traits::{
        hardware_executor::{
            HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
            HardwareExecutionStatus, IHardwareExecutor,
        },
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
    },
};

pub struct GuardedHardwareActuator {
    matrix: DeviceCapabilityMatrix,
    backend: Option<Arc<dyn IHardwareExecutor>>,
}

impl GuardedHardwareActuator {
    pub fn new(
        matrix: DeviceCapabilityMatrix,
        backend: Option<Arc<dyn IHardwareExecutor>>,
    ) -> Self {
        Self { matrix, backend }
    }
}

impl ISafeHardwareActuator for GuardedHardwareActuator {
    fn apply_authorized(
        &self,
        request: &HardwareExecutionRequest,
        permit: &ActuationPermit,
        now_unix_ms: u64,
    ) -> ContractResult<HardwareExecutionResult> {
        let rejected = |reason: &str| HardwareExecutionResult {
            device_id: request.device_id.clone(),
            status: HardwareExecutionStatus::NotSupported {
                reason: reason.into(),
            },
            latency_ms: 0,
        };
        let required = match request.hint {
            HardwareExecutionHint::WorkloadPreference(_) => HardwareCapability::WorkloadHint,
            HardwareExecutionHint::MaxPowerWatts(_) => HardwareCapability::VendorControl,
            HardwareExecutionHint::GraphicsMode
            | HardwareExecutionHint::ComputeMode
            | HardwareExecutionHint::AutoMode => HardwareCapability::ProviderSelection,
        };
        if permit.model_only {
            return Ok(HardwareExecutionResult {
                device_id: request.device_id.clone(),
                status: HardwareExecutionStatus::Skipped {
                    reason: "model-only permit does not actuate hardware".into(),
                },
                latency_ms: 0,
            });
        }
        if request.device_id != self.matrix.device_id
            || permit.device_id != request.device_id
            || permit.capability != required
            || now_unix_ms >= permit.expires_at_unix_ms
            || !self.matrix.supports(required)
        {
            return Ok(rejected("device, capability or permit mismatch/expiry"));
        }
        match &self.backend {
            Some(backend) => backend.apply(request),
            None => Ok(rejected("no actuator backend bound")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::hardware::{
        capability::{HardwareCapabilityEntry, HardwareCapabilitySet},
        matrix::{HardwareVendor, HostOs},
    };

    struct MockBackend;
    impl IHardwareExecutor for MockBackend {
        fn apply(
            &self,
            request: &HardwareExecutionRequest,
        ) -> ContractResult<HardwareExecutionResult> {
            Ok(HardwareExecutionResult {
                device_id: request.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: 1,
            })
        }
        fn release(&self, _device_id: &str) -> ContractResult<HardwareExecutionResult> {
            panic!("release not expected")
        }
        fn executor_name(&self) -> &'static str {
            "mock"
        }
    }
    fn actuator() -> GuardedHardwareActuator {
        GuardedHardwareActuator::new(
            DeviceCapabilityMatrix {
                os: HostOs::Linux,
                vendor: HardwareVendor::Other("mock".into()),
                device_id: "gpu:mock".into(),
                capabilities: HardwareCapabilitySet::new("gpu:mock").with(
                    HardwareCapabilityEntry::supported(HardwareCapability::WorkloadHint),
                ),
                thermal_limit_celsius: None,
            },
            Some(Arc::new(MockBackend)),
        )
    }
    #[test]
    fn only_live_matching_permit_reaches_mock_backend() {
        let request = HardwareExecutionRequest {
            device_id: "gpu:mock".into(),
            hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
        };
        let mut permit = ActuationPermit {
            device_id: "gpu:mock".into(),
            capability: HardwareCapability::WorkloadHint,
            expires_at_unix_ms: 100,
            model_only: true,
        };
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 10)
                .unwrap()
                .status,
            HardwareExecutionStatus::Skipped { .. }
        ));
        permit.model_only = false;
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 100)
                .unwrap()
                .status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 10)
                .unwrap()
                .status,
            HardwareExecutionStatus::Applied
        ));
    }
}
