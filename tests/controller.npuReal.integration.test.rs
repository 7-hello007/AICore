//! NPU 真实执行链集成测试。

use std::sync::Arc;

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
}

impl IHardwareExecutor for MockExecutor {
    fn apply(
        &self,
        req: &HardwareExecutionRequest,
    ) -> aicore_contracts::errors::ContractResult<HardwareExecutionResult> {
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
    HardwareCapabilitySet::new("npu:accel0").with(HardwareCapabilityEntry::supported(
        HardwareCapability::DeviceIdentity,
    ))
}

#[test]
fn npu_route_executes_with_executor() {
    let exec = Arc::new(MockExecutor {
        status: HardwareExecutionStatus::Applied,
    });
    let binder = DefaultRuntimeBinder::new()
        .with_hardware(npu_caps())
        .with_npu_executor(exec);

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
