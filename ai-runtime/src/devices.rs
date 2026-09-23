use std::sync::Arc;

use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, InferenceResponse, ModelHandle, ModelKind, RuntimeCapability,
        RuntimeHealth,
    },
    errors::{ContractError, ContractResult},
    traits::ai_runtime::IAiRuntime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDevice {
    Cpu,
    Gpu,
    Npu,
}

pub struct DeviceRuntime {
    pub device: ModelDevice,
    pub device_id: String,
    pub model_kind: ModelKind,
    pub handle: ModelHandle,
    pub runtime: Arc<dyn IAiRuntime>,
}

pub struct DeviceInference {
    pub device: ModelDevice,
    pub device_id: String,
    pub fallback: bool,
    pub response: InferenceResponse,
}

/// Routes only among explicitly registered, healthy model runtimes. This is
/// model placement, not a GPU/NPU actuator or proof of hardware execution.
pub struct DeviceRuntimeCatalog {
    entries: Vec<DeviceRuntime>,
}

impl DeviceRuntimeCatalog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn register(&mut self, entry: DeviceRuntime) -> ContractResult<()> {
        if entry.device_id.trim().is_empty()
            || self
                .entries
                .iter()
                .any(|existing| existing.device_id == entry.device_id)
        {
            return Err(ContractError::ContractInvalid(
                "empty or duplicate model device ID".into(),
            ));
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn infer(
        &self,
        preferred: ModelDevice,
        kind: ModelKind,
        request: &InferenceRequest,
    ) -> ContractResult<DeviceInference> {
        let mut last_error = None;
        for device in [preferred, ModelDevice::Cpu] {
            if device == ModelDevice::Cpu && preferred == ModelDevice::Cpu && last_error.is_some() {
                break;
            }
            for entry in self
                .entries
                .iter()
                .filter(|entry| entry.device == device && entry.model_kind == kind)
            {
                if !matches!(entry.runtime.health(), RuntimeHealth::Healthy { loaded_models } if loaded_models > 0)
                    || !entry
                        .runtime
                        .capabilities()
                        .contains(&required_capability(kind))
                {
                    continue;
                }
                match entry.runtime.infer(&entry.handle, request) {
                    Ok(response) => {
                        return Ok(DeviceInference {
                            device,
                            device_id: entry.device_id.clone(),
                            fallback: device != preferred,
                            response,
                        })
                    },
                    Err(error) => last_error = Some(error),
                }
            }
            if preferred == ModelDevice::Cpu {
                break;
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ContractError::NotSupported(
                "no healthy runtime for preferred device or CPU fallback".into(),
            )
        }))
    }
}

impl Default for DeviceRuntimeCatalog {
    fn default() -> Self {
        Self::new()
    }
}

fn required_capability(kind: ModelKind) -> RuntimeCapability {
    match kind {
        ModelKind::FeatureWeighted => RuntimeCapability::FeatureInference,
        ModelKind::Onnx => RuntimeCapability::OnnxInference,
        ModelKind::LlamaCpp => RuntimeCapability::LlamaCppInference,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::ai::runtime::{InferenceLabel, ModelSpec};

    struct MockRuntime {
        healthy: bool,
    }

    impl IAiRuntime for MockRuntime {
        fn load_model(&self, _spec: &ModelSpec) -> ContractResult<ModelHandle> {
            Err(ContractError::NotSupported("mock load".into()))
        }
        fn unload(&self, _handle: &ModelHandle) -> ContractResult<()> {
            Ok(())
        }
        fn infer(
            &self,
            _handle: &ModelHandle,
            _request: &InferenceRequest,
        ) -> ContractResult<InferenceResponse> {
            Ok(InferenceResponse {
                labels: vec![InferenceLabel {
                    label: "mock".into(),
                    score: 1.0,
                }],
                latency_ms: 1,
                model_version: "1".into(),
            })
        }
        fn capabilities(&self) -> Vec<RuntimeCapability> {
            vec![RuntimeCapability::FeatureInference]
        }
        fn health(&self) -> RuntimeHealth {
            if self.healthy {
                RuntimeHealth::Healthy { loaded_models: 1 }
            } else {
                RuntimeHealth::NotReady
            }
        }
        fn runtime_name(&self) -> &'static str {
            "mock_runtime"
        }
    }

    fn entry(device: ModelDevice, healthy: bool) -> DeviceRuntime {
        DeviceRuntime {
            device,
            device_id: format!("{:?}-mock", device),
            model_kind: ModelKind::FeatureWeighted,
            handle: ModelHandle {
                handle_id: "mock".into(),
                model_name: "mock".into(),
                model_version: "1".into(),
                loaded_at_unix_ms: 0,
            },
            runtime: Arc::new(MockRuntime { healthy }),
        }
    }

    #[test]
    fn unhealthy_gpu_falls_back_to_real_registered_cpu_runtime() {
        let mut catalog = DeviceRuntimeCatalog::new();
        catalog.register(entry(ModelDevice::Gpu, false)).unwrap();
        catalog.register(entry(ModelDevice::Cpu, true)).unwrap();
        let result = catalog
            .infer(
                ModelDevice::Gpu,
                ModelKind::FeatureWeighted,
                &InferenceRequest::default(),
            )
            .unwrap();
        assert_eq!(result.device, ModelDevice::Cpu);
        assert!(result.fallback);
        assert_eq!(result.response.labels[0].label, "mock");
    }

    #[test]
    fn absent_backend_is_not_claimed_as_executed() {
        let mut catalog = DeviceRuntimeCatalog::new();
        catalog.register(entry(ModelDevice::Npu, false)).unwrap();
        assert!(matches!(
            catalog.infer(
                ModelDevice::Npu,
                ModelKind::FeatureWeighted,
                &InferenceRequest::default()
            ),
            Err(ContractError::NotSupported(_))
        ));
    }
}
