use std::sync::Arc;
use std::time::Instant;

use aicore_contracts::{
    ai::runtime::ModelHandle,
    errors::ContractResult,
    hardware::capability::{HardwareCapability, HardwareCapabilitySet, HardwareDeviceKind},
    runtime::route::RuntimeRouteSource,
    traits::hardware_executor::{
        HardwareExecutionHint, HardwareExecutionRequest, IHardwareExecutor,
    },
    traits::{
        ai_runtime::IAiRuntime,
        runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
    },
};

/// Default Runtime Binder.
///
/// Semantics:
/// - **Native** `executed: false` (not taken over).
/// - **AiRuntime / Cpu** binding `IAiRuntime`; implementation of inference when available.
/// - ** Gpu / Npu** must have a match for hardware capacity and executor.
/// - Missing capability or execurator →`executed: false`, retreating upper layers.
/// - Any execution failed `executed: false`, getting the upper layer back in native.
pub struct DefaultRuntimeBinder {
    runtime: Option<Arc<dyn IAiRuntime>>,
    model_handle: Option<ModelHandle>,
    hardware: Vec<HardwareCapabilitySet>,
    gpu_executor: Option<Arc<dyn IHardwareExecutor>>,
    npu_executor: Option<Arc<dyn IHardwareExecutor>>,
    gpu_actuator: Option<(Arc<dyn ISafeHardwareActuator>, ActuationPermit)>,
    npu_actuator: Option<(Arc<dyn ISafeHardwareActuator>, ActuationPermit)>,
}

impl DefaultRuntimeBinder {
    pub fn new() -> Self {
        Self {
            runtime: None,
            model_handle: None,
            hardware: Vec::new(),
            gpu_executor: None,
            npu_executor: None,
            gpu_actuator: None,
            npu_actuator: None,
        }
    }

    /// Fit AI Runtime backend.
    pub fn with_ai_runtime(mut self, runtime: Arc<dyn IAiRuntime>, handle: ModelHandle) -> Self {
        self.runtime = Some(runtime);
        self.model_handle = Some(handle);
        self
    }

    /// List of installed hardware capabilities (for determination by GPU / NPU route).
    pub fn with_hardware(mut self, caps: HardwareCapabilitySet) -> Self {
        self.hardware.push(caps);
        self
    }

    pub fn with_gpu_executor(mut self, exec: Arc<dyn IHardwareExecutor>) -> Self {
        self.gpu_executor = Some(exec);
        self
    }

    pub fn with_npu_executor(mut self, exec: Arc<dyn IHardwareExecutor>) -> Self {
        self.npu_executor = Some(exec);
        self
    }

    /// Hardware authorization is supplied by the control layer, never minted by routing.
    pub fn with_gpu_actuator(
        mut self,
        actuator: Arc<dyn ISafeHardwareActuator>,
        permit: ActuationPermit,
    ) -> Self {
        self.gpu_actuator = Some((actuator, permit));
        self
    }

    pub fn with_npu_actuator(
        mut self,
        actuator: Arc<dyn ISafeHardwareActuator>,
        permit: ActuationPermit,
    ) -> Self {
        self.npu_actuator = Some((actuator, permit));
        self
    }

    pub fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn has_gpu_executor(&self) -> bool {
        self.gpu_executor.is_some()
    }

    pub fn has_npu_executor(&self) -> bool {
        self.npu_executor.is_some()
    }

    pub fn hardware_count(&self) -> usize {
        self.hardware.len()
    }

    fn gpu_capability_present(&self) -> bool {
        self.hardware.iter().any(|h| {
            h.device_kind == HardwareDeviceKind::Gpu
                && h.is_supported(HardwareCapability::DeviceIdentity)
        })
    }

    fn npu_capability_present(&self) -> bool {
        self.hardware.iter().any(|h| {
            h.device_kind == HardwareDeviceKind::Npu
                && h.is_supported(HardwareCapability::DeviceIdentity)
        })
    }
}

impl Default for DefaultRuntimeBinder {
    fn default() -> Self {
        Self::new()
    }
}

impl IRuntimeBinder for DefaultRuntimeBinder {
    fn preparation_safe(&self) -> bool {
        self.gpu_actuator.is_none()
            && self.npu_actuator.is_none()
            && self.gpu_executor.is_none()
            && self.npu_executor.is_none()
    }

    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult> {
        let start = Instant::now();

        match request.route.source {
            RuntimeRouteSource::Native => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Native,
                executed: false,
                inference: None,
                reason: "route native: no backend needed".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            }),

            RuntimeRouteSource::AiRuntime | RuntimeRouteSource::Cpu => {
                self.bind_ai_runtime(request, start)
            },

            RuntimeRouteSource::Gpu => self.bind_gpu(request, start),

            RuntimeRouteSource::Npu => self.bind_npu(request, start),
        }
    }

    fn name(&self) -> &'static str {
        "default_runtime_binder"
    }

    fn loaded_models(&self) -> Vec<ModelHandle> {
        self.model_handle.iter().cloned().collect()
    }
}

impl DefaultRuntimeBinder {
    fn bind_ai_runtime(
        &self,
        request: &BindingRequest,
        start: Instant,
    ) -> ContractResult<BindingResult> {
        let runtime = match &self.runtime {
            Some(r) => r,
            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: "no IAiRuntime backend registered".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let handle = match &self.model_handle {
            Some(h) => h,
            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: "no model loaded in binder".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        // Binding a model is not an inference operation.
        let inference_req = match &request.inference {
            Some(r) => r,
            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: format!("{} available, no inference payload", runtime.runtime_name()),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        // Exercising inference
        match runtime.infer(handle, inference_req) {
            Ok(resp) => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: request.route.source,
                executed: true,
                inference: Some(resp),
                reason: format!("executed via {}", runtime.runtime_name()),
                latency_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => {
                tracing::warn!(
                    component = "runtime_binder",
                    event = "inference_failed",
                    runtime = runtime.runtime_name(),
                    error = %e,
                    "binder inference failed; will fall back to native"
                );
                Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: format!("inference failed: {}", e),
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            },
        }
    }

    fn bind_gpu(&self, request: &BindingRequest, start: Instant) -> ContractResult<BindingResult> {
        if !self.gpu_capability_present() {
            return Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Gpu,
                executed: false,
                inference: None,
                reason: "gpu route but no GPU capability registered".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        let hint = match request.context.workload_type {
            aicore_contracts::context::workload::WorkloadType::AiInference => {
                HardwareExecutionHint::WorkloadPreference("compute".into())
            },
            aicore_contracts::context::workload::WorkloadType::Rendering
            | aicore_contracts::context::workload::WorkloadType::VideoEditing => {
                HardwareExecutionHint::WorkloadPreference("graphics".into())
            },
            _ => HardwareExecutionHint::WorkloadPreference("auto".into()),
        };
        self.bind_authorized_hardware(
            request,
            start,
            HardwareDeviceKind::Gpu,
            hint,
            &self.gpu_actuator,
        )
    }

    fn bind_npu(&self, request: &BindingRequest, start: Instant) -> ContractResult<BindingResult> {
        if !self.npu_capability_present() {
            return Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Npu,
                executed: false,
                inference: None,
                reason: "npu route but no NPU capability registered".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        self.bind_authorized_hardware(
            request,
            start,
            HardwareDeviceKind::Npu,
            HardwareExecutionHint::AutoMode,
            &self.npu_actuator,
        )
    }

    fn bind_authorized_hardware(
        &self,
        request: &BindingRequest,
        start: Instant,
        kind: HardwareDeviceKind,
        hint: HardwareExecutionHint,
        binding: &Option<(Arc<dyn ISafeHardwareActuator>, ActuationPermit)>,
    ) -> ContractResult<BindingResult> {
        let mut result = BindingResult {
            route_id: request.route.route_id.clone(),
            source: request.route.source,
            executed: false,
            inference: None,
            reason:
                "hardware capability present but no executor registered with guarded authorization"
                    .into(),
            latency_ms: 0,
        };
        if let Some((actuator, permit)) = binding {
            if self.hardware.iter().any(|caps| {
                caps.device_name == permit.device_id
                    && caps.device_kind == kind
                    && caps.is_supported(HardwareCapability::DeviceIdentity)
            }) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(u64::MAX);
                let hardware_request = HardwareExecutionRequest {
                    device_id: permit.device_id.clone(),
                    hint,
                };
                match actuator.apply_authorized(&hardware_request, permit, now) {
                    Ok(outcome) => {
                        result.executed = outcome.device_id == hardware_request.device_id
                            && outcome.status.is_applied();
                        result.reason = format!(
                            "guarded hardware executor: device={} hint={:?} status={:?}",
                            hardware_request.device_id, hardware_request.hint, outcome.status
                        );
                    },
                    Err(error) => {
                        result.reason = format!(
                            "guarded hardware executor: device={} error={error}",
                            hardware_request.device_id
                        )
                    },
                }
            } else {
                result.reason = "guarded permit has no matching discovered device".into();
            }
        }
        result.latency_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }
}

#[cfg(test)]
mod p0_tests {
    use super::*;
    use aicore_contracts::{
        context::{snapshot::ContextSnapshot, workload::WorkloadType},
        errors::ContractResult,
        runtime::route::RuntimeRoute,
        traits::hardware_executor::{HardwareExecutionResult, HardwareExecutionStatus},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct LegacyBackend(Arc<AtomicUsize>);
    impl IHardwareExecutor for LegacyBackend {
        fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: 0,
            })
        }
        fn release(&self, _: &str) -> ContractResult<HardwareExecutionResult> {
            unreachable!()
        }
        fn executor_name(&self) -> &'static str {
            "legacy"
        }
    }
    #[test]
    fn p0_binder_never_writes_through_unpermitted_legacy_backend() {
        for source in [RuntimeRouteSource::Gpu, RuntimeRouteSource::Npu] {
            let writes = Arc::new(AtomicUsize::new(0));
            let id = if source == RuntimeRouteSource::Gpu {
                "gpu:test"
            } else {
                "npu:test"
            };
            let caps = HardwareCapabilitySet::new(id)
                .with_device_kind(if source == RuntimeRouteSource::Gpu {
                    HardwareDeviceKind::Gpu
                } else {
                    HardwareDeviceKind::Npu
                })
                .with(
                    aicore_contracts::hardware::capability::HardwareCapabilityEntry::supported(
                        HardwareCapability::DeviceIdentity,
                    ),
                );
            let backend = Arc::new(LegacyBackend(writes.clone()));
            let binder = DefaultRuntimeBinder::new()
                .with_hardware(caps)
                .with_gpu_executor(backend.clone())
                .with_npu_executor(backend);
            let result = binder
                .bind_and_execute(&BindingRequest {
                    route: RuntimeRoute {
                        route_id: "p0".into(),
                        workload_type: WorkloadType::AiInference,
                        source,
                        applied: false,
                        reason: "test".into(),
                        confidence: 1.0,
                        decided_at_unix_secs: 0,
                    },
                    context: ContextSnapshot::default(),
                    inference: None,
                })
                .unwrap();
            assert!(
                !result.executed,
                "binder cannot mint hardware authorization"
            );
            assert_eq!(writes.load(Ordering::SeqCst), 0);
        }
    }
}
