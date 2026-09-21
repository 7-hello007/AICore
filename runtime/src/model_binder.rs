use std::sync::Arc;
use std::time::Instant;

use aicore_contracts::{
    ai::runtime::ModelHandle,
    errors::ContractResult,
    runtime::route::RuntimeRouteSource,
    traits::{
        ai_runtime::IAiRuntime,
        runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
    },
};

/// 模型模式 Runtime Binder。
///
/// GPU / NPU 不访问真实硬件。
///
/// AI Runtime / CPU 仍然可以执行真实本地模型推理：
///
/// Model AI Runtime
///     ↓
/// local JSON model
///
/// 本身不修改宿主机状态。
pub struct ModelRuntimeBinder {
    runtime: Option<Arc<dyn IAiRuntime>>,
    model_handle: Option<ModelHandle>,
}

impl ModelRuntimeBinder {
    pub fn new() -> Self {
        Self {
            runtime: None,
            model_handle: None,
        }
    }

    pub fn with_ai_runtime(mut self, runtime: Arc<dyn IAiRuntime>, handle: ModelHandle) -> Self {
        self.runtime = Some(runtime);
        self.model_handle = Some(handle);
        self
    }

    fn bind_ai_runtime(
        &self,
        request: &BindingRequest,
        start: Instant,
    ) -> ContractResult<BindingResult> {
        let runtime = match &self.runtime {
            Some(runtime) => runtime,

            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: "model binder has no AI runtime".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let handle = match &self.model_handle {
            Some(handle) => handle,

            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: "model binder has no loaded model".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let inference = match &request.inference {
            Some(inference) => inference,

            None => {
                return Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: true,
                    inference: None,
                    reason: format!(
                        "model binder attached to {} without inference",
                        runtime.runtime_name()
                    ),
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        match runtime.infer(handle, inference) {
            Ok(response) => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: request.route.source,
                executed: true,
                inference: Some(response),
                reason: format!("model inference executed by {}", runtime.runtime_name()),
                latency_ms: start.elapsed().as_millis() as u64,
            }),

            Err(error) => {
                tracing::warn!(
                    component = "model_runtime_binder",
                    event = "model_inference_failed",
                    error = %error,
                    "model inference failed"
                );

                Ok(BindingResult {
                    route_id: request.route.route_id.clone(),
                    source: request.route.source,
                    executed: false,
                    inference: None,
                    reason: format!("model inference failed: {}", error),
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            },
        }
    }
}

impl Default for ModelRuntimeBinder {
    fn default() -> Self {
        Self::new()
    }
}

impl IRuntimeBinder for ModelRuntimeBinder {
    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult> {
        let start = Instant::now();

        match request.route.source {
            /*
             * ----------------------------------------------------
             * Native
             * ----------------------------------------------------
             */
            RuntimeRouteSource::Native => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Native,
                executed: false,
                inference: None,
                reason: "model mode: native route requires no backend".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            }),

            /*
             * ----------------------------------------------------
             * AI Runtime / CPU
             * ----------------------------------------------------
             */
            RuntimeRouteSource::AiRuntime | RuntimeRouteSource::Cpu => {
                self.bind_ai_runtime(request, start)
            },

            /*
             * ----------------------------------------------------
             * GPU
             *
             * 只模拟，不访问真实 GPU。
             * ----------------------------------------------------
             */
            RuntimeRouteSource::Gpu => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Gpu,
                executed: true,
                inference: None,
                reason: "model mode: GPU execution simulated".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            }),

            /*
             * ----------------------------------------------------
             * NPU
             *
             * 只模拟，不访问真实 NPU。
             * ----------------------------------------------------
             */
            RuntimeRouteSource::Npu => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: RuntimeRouteSource::Npu,
                executed: true,
                inference: None,
                reason: "model mode: NPU execution simulated".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn name(&self) -> &'static str {
        "model_runtime_binder"
    }

    fn loaded_models(&self) -> Vec<ModelHandle> {
        self.model_handle.iter().cloned().collect()
    }
}
