//! GPU 真实执行链集成测试（Mock 与真实平台组合）。

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

/// Mock 硬件执行器：可配置返回状态。
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
        "mock_executor"
    }
}

fn gpu_route() -> RuntimeRoute {
    RuntimeRoute {
        route_id: "r-gpu".to_string(),
        workload_type: aicore_contracts::context::workload::WorkloadType::Rendering,
        source: RuntimeRouteSource::Gpu,
        applied: true,
        reason: "test".to_string(),
        confidence: 0.9,
        decided_at_unix_secs: 1_700_000_000,
    }
}

fn gpu_caps() -> HardwareCapabilitySet {
    HardwareCapabilitySet::new("drm:card0").with(HardwareCapabilityEntry::supported(
        HardwareCapability::DeviceIdentity,
    ))
}

#[test]
fn gpu_route_with_capability_and_applied_executor() {
    let exec = Arc::new(MockExecutor {
        status: HardwareExecutionStatus::Applied,
    });
    let binder = DefaultRuntimeBinder::new()
        .with_hardware(gpu_caps())
        .with_gpu_executor(exec);

    assert!(binder.has_gpu_executor());

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: gpu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(binding.executed);
    assert_eq!(binding.source, RuntimeRouteSource::Gpu);
    assert!(binding.reason.contains("gpu executor"));
}

#[test]
fn gpu_route_with_not_supported_executor_does_not_execute() {
    let exec = Arc::new(MockExecutor {
        status: HardwareExecutionStatus::NotSupported {
            reason: "no sysfs".to_string(),
        },
    });
    let binder = DefaultRuntimeBinder::new()
        .with_hardware(gpu_caps())
        .with_gpu_executor(exec);

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: gpu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(!binding.executed);
    assert!(binding.reason.contains("NotSupported") || binding.reason.contains("gpu executor"));
}

#[test]
fn gpu_route_with_capability_but_no_executor_does_not_execute() {
    let binder = DefaultRuntimeBinder::new().with_hardware(gpu_caps());

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: gpu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(!binding.executed);
    assert!(binding.reason.contains("no executor registered"));
}

#[test]
fn gpu_route_without_capability_does_not_execute() {
    let binder = DefaultRuntimeBinder::new();

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: gpu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(!binding.executed);
    assert!(binding.reason.contains("no GPU capability"));
}

#[test]
fn already_in_state_counts_as_executed() {
    let exec = Arc::new(MockExecutor {
        status: HardwareExecutionStatus::AlreadyInState,
    });
    let binder = DefaultRuntimeBinder::new()
        .with_hardware(gpu_caps())
        .with_gpu_executor(exec);

    let binding = binder
        .bind_and_execute(&BindingRequest {
            route: gpu_route(),
            context: ContextSnapshot::default(),
            inference: None,
        })
        .unwrap();

    assert!(binding.executed);
}
