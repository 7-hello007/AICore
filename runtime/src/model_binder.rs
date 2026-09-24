use std::sync::Arc;
use std::time::Instant;

use aicore_ai_runtime::{DeviceRuntimeCatalog, ModelDevice};
use aicore_contracts::{
    ai::runtime::{ModelHandle, ModelKind},
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
    device_catalog: Option<Arc<DeviceRuntimeCatalog>>,
}

impl ModelRuntimeBinder {
    pub fn new() -> Self {
        Self {
            runtime: None,
            model_handle: None,
            device_catalog: None,
        }
    }

    pub fn with_ai_runtime(mut self, runtime: Arc<dyn IAiRuntime>, handle: ModelHandle) -> Self {
        self.runtime = Some(runtime);
        self.model_handle = Some(handle);
        self
    }

    pub fn with_device_catalog(mut self, catalog: Arc<DeviceRuntimeCatalog>) -> Self {
        self.device_catalog = Some(catalog);
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

        let result = if let Some(catalog) = &self.device_catalog {
            catalog
                .infer(ModelDevice::Cpu, ModelKind::FeatureWeighted, inference)
                .map(|selected| selected.response)
        } else {
            runtime.infer(handle, inference)
        };
        match result {
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

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_ai_runtime::{DeviceRuntime, PureRustRuntime};
    use aicore_contracts::{
        ai::runtime::{InferenceRequest, ModelSource, ModelSpec},
        context::{snapshot::ContextSnapshot, workload::WorkloadType},
        runtime::route::RuntimeRoute,
    };

    #[test]
    fn cpu_route_uses_registered_local_json_model_without_hardware_backend() {
        let runtime = Arc::new(PureRustRuntime::new());
        let handle = runtime
            .load_model(&ModelSpec {
                name: "policy_classifier_v1".into(),
                version: "1.0.0".into(),
                kind: ModelKind::FeatureWeighted,
                source: ModelSource::Inline {
                    json: include_str!("../../models/policy_classifier_v1.json").into(),
                },
                onnx_spec: None,
            })
            .unwrap();
        let mut catalog = DeviceRuntimeCatalog::new();
        catalog
            .register(DeviceRuntime {
                device: ModelDevice::Cpu,
                device_id: "local-cpu".into(),
                model_kind: ModelKind::FeatureWeighted,
                handle: handle.clone(),
                runtime: runtime.clone(),
            })
            .unwrap();
        let binder = ModelRuntimeBinder::new()
            .with_ai_runtime(runtime, handle)
            .with_device_catalog(Arc::new(catalog));
        let response = binder
            .bind_and_execute(&BindingRequest {
                route: RuntimeRoute::routed(
                    WorkloadType::AiInference,
                    RuntimeRouteSource::Cpu,
                    "isolated",
                    1.0,
                ),
                context: ContextSnapshot::default(),
                inference: Some(InferenceRequest {
                    features: vec![0.2; 4],
                    tensors: Vec::new(),
                }),
            })
            .unwrap();
        assert!(response.executed);
        assert_eq!(response.source, RuntimeRouteSource::Cpu);
        assert_eq!(response.inference.unwrap().labels.len(), 5);
    }
}
