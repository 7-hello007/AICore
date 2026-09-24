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
    },
};

/// 默认 Runtime Binder。
///
/// 语义：
/// - **Native** → `executed: false`（不接手）。
/// - **AiRuntime / Cpu** → 绑定 `IAiRuntime`；有 inference 时执行推理。
/// - **Gpu / Npu** → 必须同时具备对应 hardware capability 与 executor。
/// - 缺少 capability 或 executor → `executed: false`，让上层回退 native。
/// - 任何执行失败 → `executed: false`，让上层回退 native。
pub struct DefaultRuntimeBinder {
    runtime: Option<Arc<dyn IAiRuntime>>,
    model_handle: Option<ModelHandle>,
    hardware: Vec<HardwareCapabilitySet>,
    gpu_executor: Option<Arc<dyn IHardwareExecutor>>,
    npu_executor: Option<Arc<dyn IHardwareExecutor>>,
}

impl DefaultRuntimeBinder {
    pub fn new() -> Self {
        Self {
            runtime: None,
            model_handle: None,
            hardware: Vec::new(),
            gpu_executor: None,
            npu_executor: None,
        }
    }

    /// 装配 AI Runtime backend。
    pub fn with_ai_runtime(mut self, runtime: Arc<dyn IAiRuntime>, handle: ModelHandle) -> Self {
        self.runtime = Some(runtime);
        self.model_handle = Some(handle);
        self
    }

    /// 装配硬件能力清单（供 GPU / NPU route 判定）。
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

        // 执行推理
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

        let exec = match &self.gpu_executor {
            Some(e) => e,
            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: RuntimeRouteSource::Gpu,
                    executed: false,
                    inference: None,
                    reason: "gpu capability present but no executor registered".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let Some(device_id) = self
            .hardware
            .iter()
            .find(|h| {
                h.device_kind == HardwareDeviceKind::Gpu
                    && h.is_supported(HardwareCapability::DeviceIdentity)
            })
            .map(|h| h.device_name.clone())
        else {
            return Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Gpu,
                executed: false,
                inference: None,
                reason: "no identified GPU device".into(),
                latency_ms: start.elapsed().as_millis() as u64,
            });
        };
        let hint = match request.context.workload_type {
            aicore_contracts::context::workload::WorkloadType::AiInference => {
                HardwareExecutionHint::WorkloadPreference("compute".to_string())
            },
            aicore_contracts::context::workload::WorkloadType::Rendering
            | aicore_contracts::context::workload::WorkloadType::VideoEditing => {
                HardwareExecutionHint::WorkloadPreference("graphics".to_string())
            },
            _ => HardwareExecutionHint::WorkloadPreference("auto".to_string()),
        };
        let exec_req = HardwareExecutionRequest { device_id, hint };

        match exec.apply(&exec_req) {
            Ok(result) => {
                let executed = result.status.is_applied();
                Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: RuntimeRouteSource::Gpu,
                    executed,
                    inference: None,
                    reason: format!("gpu executor: {:?}", result.status),
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            },
            Err(e) => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Gpu,
                executed: false,
                inference: None,
                reason: format!("gpu executor error: {}", e),
                latency_ms: start.elapsed().as_millis() as u64,
            }),
        }
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

        let exec = match &self.npu_executor {
            Some(e) => e,
            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: RuntimeRouteSource::Npu,
                    executed: false,
                    inference: None,
                    reason: "npu capability present but no executor registered".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let Some(device_id) = self
            .hardware
            .iter()
            .find(|h| {
                h.device_kind == HardwareDeviceKind::Npu
                    && h.is_supported(HardwareCapability::DeviceIdentity)
            })
            .map(|h| h.device_name.clone())
        else {
            return Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Npu,
                executed: false,
                inference: None,
                reason: "no identified NPU device".into(),
                latency_ms: start.elapsed().as_millis() as u64,
            });
        };

        let exec_req = HardwareExecutionRequest {
            device_id,
            hint: HardwareExecutionHint::AutoMode,
        };

        match exec.apply(&exec_req) {
            Ok(result) => {
                let executed = result.status.is_applied();
                Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: RuntimeRouteSource::Npu,
                    executed,
                    inference: None,
                    reason: format!("npu executor: {:?}", result.status),
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            },
            Err(e) => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Npu,
                executed: false,
                inference: None,
                reason: format!("npu executor error: {}", e),
                latency_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }
}
