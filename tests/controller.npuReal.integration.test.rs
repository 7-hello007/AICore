//! Memory-only NPU routing with the production binder and guarded actuator.
//! No native hardware discovery, setter, or command is executed.

use aicore_contracts::{
    hardware::matrix::{DeviceCapabilityMatrix, HardwareVendor, HostOs},
    traits::{
        hardware_executor::{HardwareExecutionHint, HardwareStateSnapshot},
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
    },
};
use aicore_core::safe_actuator::GuardedHardwareActuator;
use std::sync::{Arc, Mutex};

use aicore_contracts::{
    context::snapshot::ContextSnapshot,
    hardware::capability::{HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet},
    runtime::route::{RuntimeRoute, RuntimeRouteSource},
    traits::{
        hardware_executor::{
            HardwareExecutionRequest, HardwareExecutionResult, HardwareExecutionStatus,
            IHardwareExecutor,
        },
        runtime_binder::{BindingRequest, IRuntimeBinder},
    },
};
use aicore_runtime_router::DefaultRuntimeBinder;

struct MockExecutor {
    status: HardwareExecutionStatus,
    state: Mutex<Option<HardwareExecutionHint>>,
}

impl IHardwareExecutor for MockExecutor {
    fn is_model_only(&self) -> bool {
        true
    }
    fn supports_reversible_state(&self) -> bool {
        true
    }
    fn read_hardware_state(
        &self,
        id: &str,
    ) -> aicore_contracts::errors::ContractResult<HardwareStateSnapshot> {
        Ok(HardwareStateSnapshot {
            device_id: id.into(),
            hint: self.state.lock().unwrap().clone(),
        })
    }
    fn restore_hardware_state(
        &self,
        state: &HardwareStateSnapshot,
    ) -> aicore_contracts::errors::ContractResult<HardwareExecutionResult> {
        *self.state.lock().unwrap() = state.hint.clone();
        Ok(HardwareExecutionResult {
            device_id: state.device_id.clone(),
            status: HardwareExecutionStatus::Applied,
            latency_ms: 0,
        })
    }

    fn apply(
        &self,
        req: &HardwareExecutionRequest,
    ) -> aicore_contracts::errors::ContractResult<HardwareExecutionResult> {
        if self.status.is_applied() {
            *self.state.lock().unwrap() = Some(req.hint.clone());
        }
        Ok(HardwareExecutionResult {
            device_id: req.device_id.clone(),
            status: self.status.clone(),
            latency_ms: 1,
        })
    }
    fn release(
        &self,
        device_id: &str,
    ) -> aicore_contracts::errors::ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: device_id.to_string(),
            status: HardwareExecutionStatus::Applied,
            latency_ms: 1,
        })
    }
    fn executor_name(&self) -> &'static str {
        "mock_npu_executor"
    }
}

fn npu_route() -> RuntimeRoute {
    RuntimeRoute {
        route_id: "r-npu".to_string(),
        workload_type: aicore_contracts::context::workload::WorkloadType::AiInference,
        source: RuntimeRouteSource::Npu,
        applied: true,
        reason: "test".to_string(),
        confidence: 0.9,
        decided_at_unix_secs: 1_700_000_000,
    }
}

fn npu_caps() -> HardwareCapabilitySet {
    HardwareCapabilitySet::new("npu:accel0")
        .with_device_kind(aicore_contracts::hardware::capability::HardwareDeviceKind::Npu)
        .with(HardwareCapabilityEntry::supported(
            HardwareCapability::DeviceIdentity,
        ))
        .with(HardwareCapabilityEntry::supported(
            HardwareCapability::ProviderSelection,
        ))
}

fn permit() -> ActuationPermit {
    ActuationPermit {
        device_id: "npu:accel0".into(),
        capability: HardwareCapability::ProviderSelection,
        expires_at_unix_ms: u64::MAX,
        model_only: true,
    }
}
fn guarded_executor(executor: Arc<dyn IHardwareExecutor>) -> Arc<dyn ISafeHardwareActuator> {
    Arc::new(GuardedHardwareActuator::new(
        DeviceCapabilityMatrix {
            os: HostOs::Windows,
            vendor: HardwareVendor::Other("model test".into()),
            device_id: "npu:accel0".into(),
            capabilities: npu_caps(),
            thermal_limit_celsius: None,
        },
        Some(executor),
    ))
}

#[test]
fn npu_route_executes_with_executor() {
    let exec = Arc::new(MockExecutor {
        status: HardwareExecutionStatus::Applied,
        state: Mutex::new(None),
    });
    let binder = DefaultRuntimeBinder::new()
        .with_hardware(npu_caps())
        .with_npu_actuator(guarded_executor(exec), permit());

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: npu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(binding.executed);
    assert_eq!(binding.source, RuntimeRouteSource::Npu);
}

#[test]
fn npu_route_no_capability_degraded() {
    let binder = DefaultRuntimeBinder::new();
    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: npu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();
    assert!(!binding.executed);
    assert!(binding.reason.contains("no NPU capability"));
}
