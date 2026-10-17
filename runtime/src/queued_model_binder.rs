//! Session-only accelerator placement. No global process, power or device setters.
use aicore_contracts::{
    ai::runtime::{InferenceRequest, ModelHandle, TensorDType, TensorInput, TensorSchema},
    errors::{ContractError, ContractResult},
    hardware::{capability::HardwareDeviceKind, matrix::DeviceCapabilityMatrix},
    policy::priority::PolicyPriority,
    runtime::route::RuntimeRouteSource,
    traits::{
        ai_runtime::IAiRuntime,
        hardware_executor::HardwareExecutionHint,
        runtime_binder::{BindingRequest, BindingResult, IRuntimeBinder},
    },
};
use aicore_core::accelerator_scheduler::{AcceleratorJob, AcceleratorScheduler};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct QueuedModelBinder {
    scheduler: AcceleratorScheduler,
    runtime: Arc<dyn IAiRuntime>,
    handle: ModelHandle,
    schema: TensorSchema,
    kind: HardwareDeviceKind,
    device_id: String,
    dispatch: Mutex<()>,
    fallback: Arc<dyn IRuntimeBinder>,
}
impl QueuedModelBinder {
    pub fn new(
        matrix: DeviceCapabilityMatrix,
        runtime: Arc<dyn IAiRuntime>,
        handle: ModelHandle,
        schema: TensorSchema,
        fallback: Arc<dyn IRuntimeBinder>,
    ) -> ContractResult<Self> {
        if !fallback.preparation_safe()
            || schema.dtype != TensorDType::F32
            || schema.shape.len() != 2
            || schema.shape[0] != 1
            || schema.shape[1] <= 0
        {
            return Err(ContractError::ContractInvalid(
                "queued model binder requires session-only fallback and static feature tensor"
                    .into(),
            ));
        }
        let scheduler = AcceleratorScheduler::new(64)?;
        let kind = matrix.capabilities.device_kind;
        let device_id = matrix.device_id.clone();
        scheduler.register_device(matrix, 1)?;
        Ok(Self {
            scheduler,
            runtime,
            handle,
            schema,
            kind,
            device_id,
            dispatch: Mutex::new(()),
            fallback,
        })
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
impl IRuntimeBinder for QueuedModelBinder {
    fn preparation_safe(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "queued_model_binder"
    }
    fn loaded_models(&self) -> Vec<ModelHandle> {
        vec![self.handle.clone()]
    }
    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult> {
        let start = Instant::now();
        let kind = match request.route.source {
            RuntimeRouteSource::Gpu => HardwareDeviceKind::Gpu,
            RuntimeRouteSource::Npu => HardwareDeviceKind::Npu,
            _ => return self.fallback.bind_and_execute(request),
        };
        let declined = |reason: String| BindingResult {
            route_id: request.route.route_id.clone(),
            source: request.route.source,
            executed: false,
            inference: None,
            reason,
            latency_ms: start.elapsed().as_millis() as u64,
        };
        if kind != self.kind {
            return Ok(declined("requested accelerator backend unavailable".into()));
        }
        let Ok(_guard) = self.dispatch.try_lock() else {
            return Ok(declined("accelerator queue busy".into()));
        };
        let Some(input) = &request.inference else {
            return Ok(declined("accelerator inference input missing".into()));
        };
        if input.features.len() != self.schema.shape[1] as usize
            || input.features.iter().any(|v| !v.is_finite())
        {
            return Ok(declined(
                "accelerator feature dimension/value mismatch".into(),
            ));
        }
        let timestamp = now();
        let expiry = timestamp.saturating_add(Duration::from_secs(10).as_millis() as u64);
        let id = format!("{}-{timestamp}", request.route.route_id);
        self.scheduler.enqueue(
            AcceleratorJob {
                id: id.clone(),
                kind,
                priority: PolicyPriority::Normal,
                expires_at_unix_ms: expiry,
                hint: HardwareExecutionHint::AutoMode,
            },
            timestamp,
        )?;
        let Some(lease) = self.scheduler.reserve(now())? else {
            self.scheduler.cancel(&id)?;
            return Ok(declined(
                "accelerator capability/admission unavailable".into(),
            ));
        };
        if lease.device_id() != self.device_id || now() >= expiry {
            return Ok(declined(
                "accelerator lease expired or device changed".into(),
            ));
        }
        let inference = InferenceRequest {
            features: Vec::new(),
            tensors: vec![TensorInput {
                name: self.schema.name.clone(),
                shape: self.schema.shape.clone(),
                data: input.features.clone(),
            }],
        };
        match self.runtime.infer(&self.handle, &inference) {
            Ok(response) if now() < expiry => Ok(BindingResult {
                route_id: request.route.route_id.clone(),
                source: request.route.source,
                executed: true,
                inference: Some(response),
                reason: format!("queued model inference executed on {}", self.device_id),
                latency_ms: start.elapsed().as_millis() as u64,
            }),
            Ok(_) => Ok(declined("accelerator result expired".into())),
            Err(error) => Ok(declined(format!("accelerator inference failed: {error}"))),
        }
    }
}

/// Explicit provider configuration. No download, implicit CPU fallback or mock.
pub fn configured_accelerator_binder(
    project: &std::path::Path,
    mut matrix: DeviceCapabilityMatrix,
    fallback: Arc<dyn IRuntimeBinder>,
) -> ContractResult<Arc<dyn IRuntimeBinder>> {
    use aicore_ai_runtime::onnx::{OnnxRuntimeBackend, PythonOnnxSessionFactory};
    use aicore_contracts::{
        ai::runtime::{ModelKind, ModelSource, ModelSpec, OnnxModelSpec},
        hardware::capability::{HardwareCapability, HardwareCapabilityEntry},
    };
    let provider = std::env::var("AICORE_ACCELERATOR_PROVIDER")
        .map_err(|e| ContractError::ConfigInvalid(e.to_string()))?;
    if !matches!(
        (provider.as_str(), matrix.capabilities.device_kind),
        ("DmlExecutionProvider", HardwareDeviceKind::Gpu)
            | ("VitisAIExecutionProvider", HardwareDeviceKind::Npu)
    ) {
        return Err(ContractError::ConfigInvalid(
            "accelerator provider/device mismatch".into(),
        ));
    }
    let device_id = std::env::var("AICORE_ACCELERATOR_DEVICE_ID")
        .ok()
        .map(|v| {
            v.parse::<u32>()
                .map_err(|e| ContractError::ConfigInvalid(e.to_string()))
        })
        .transpose()?;
    let python = std::env::var_os("AICORE_ONNX_PYTHON")
        .ok_or_else(|| ContractError::ConfigInvalid("AICORE_ONNX_PYTHON required".into()))?;
    let packages = std::env::var_os("AICORE_ACCELERATOR_PACKAGES").ok_or_else(|| {
        ContractError::ConfigInvalid("AICORE_ACCELERATOR_PACKAGES required".into())
    })?;
    let timeout = Duration::from_secs(5);
    let factory = PythonOnnxSessionFactory {
        python: python.into(),
        runner: project.join("scripts/models/onnx_intent_runner.py"),
        packages: packages.into(),
        timeout,
        device_id,
        provider_library: std::env::var_os("AICORE_ACCELERATOR_PROVIDER_LIBRARY").map(Into::into),
    };
    let runtime = Arc::new(
        OnnxRuntimeBackend::new(Arc::new(factory)).with_timeout(timeout + Duration::from_secs(1)),
    );
    let schema = TensorSchema {
        name: "features".into(),
        shape: vec![1, 4],
        dtype: TensorDType::F32,
    };
    let model_path = std::env::var_os("AICORE_ACCELERATOR_MODEL_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project.join("models/policy_classifier_v1.onnx"));
    if !model_path.is_absolute() {
        return Err(ContractError::ConfigInvalid(
            "AICORE_ACCELERATOR_MODEL_PATH must be absolute".into(),
        ));
    }
    let handle = runtime.load_model(&ModelSpec {
        name: "policy_classifier_v1".into(),
        version: "1.0.0".into(),
        kind: ModelKind::Onnx,
        source: ModelSource::File {
            path: model_path.to_string_lossy().into_owned(),
        },
        onnx_spec: Some(OnnxModelSpec {
            input_schema: vec![schema.clone()],
            output_schema: vec![TensorSchema {
                name: "probabilities".into(),
                shape: vec![1, 5],
                dtype: TensorDType::F32,
            }],
            preferred_provider: Some(provider),
        }),
    })?;
    matrix
        .capabilities
        .entries
        .push(HardwareCapabilityEntry::supported(
            HardwareCapability::ProviderSelection,
        ));
    Ok(Arc::new(QueuedModelBinder::new(
        matrix, runtime, handle, schema, fallback,
    )?))
}

impl Drop for QueuedModelBinder {
    fn drop(&mut self) {
        if let Err(error) = self.runtime.unload(&self.handle) {
            tracing::error!(%error,"accelerator session unload failed");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use aicore_ai_runtime::onnx::{MockOnnxSessionFactory, OnnxRuntimeBackend};
    use aicore_contracts::{
        ai::runtime::{ModelKind, ModelSource, ModelSpec, OnnxModelSpec},
        context::{snapshot::ContextSnapshot, workload::WorkloadType},
        hardware::{
            capability::{HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet},
            matrix::{HardwareVendor, HostOs},
        },
        runtime::route::RuntimeRoute,
    };
    fn request(source: RuntimeRouteSource) -> BindingRequest {
        BindingRequest {
            route: RuntimeRoute {
                route_id: "queue-test".into(),
                source,
                applied: true,
                workload_type: WorkloadType::Rendering,
                reason: "mock queue test".into(),
                confidence: 1.0,
                decided_at_unix_secs: 0,
            },
            context: ContextSnapshot::default(),
            inference: Some(InferenceRequest {
                features: vec![1.0, 0.0, 0.0, 0.0],
                tensors: vec![],
            }),
        }
    }
    fn binder() -> QueuedModelBinder {
        let runtime = Arc::new(OnnxRuntimeBackend::new(Arc::new(
            MockOnnxSessionFactory::with_default_test_session(),
        )));
        let schema = TensorSchema {
            name: "input".into(),
            shape: vec![1, 4],
            dtype: TensorDType::F32,
        };
        let spec = ModelSpec {
            name: "mock".into(),
            version: "1".into(),
            kind: ModelKind::Onnx,
            source: ModelSource::File {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../models/policy_classifier_v1.onnx"
                )
                .into(),
            },
            onnx_spec: Some(OnnxModelSpec {
                input_schema: vec![schema.clone()],
                output_schema: vec![TensorSchema {
                    name: "output".into(),
                    shape: vec![1, 4],
                    dtype: TensorDType::F32,
                }],
                preferred_provider: None,
            }),
        };
        let handle = runtime.load_model(&spec).expect("mock ONNX load");
        let matrix = DeviceCapabilityMatrix {
            os: HostOs::Windows,
            vendor: HardwareVendor::Other("mock".into()),
            device_id: "gpu:mock".into(),
            capabilities: HardwareCapabilitySet::new("gpu:mock")
                .with_device_kind(aicore_contracts::hardware::capability::HardwareDeviceKind::Gpu)
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::ProviderSelection,
                )),
            thermal_limit_celsius: None,
        };
        QueuedModelBinder::new(
            matrix,
            runtime,
            handle,
            schema,
            Arc::new(crate::ModelRuntimeBinder::new()),
        )
        .expect("queue registration")
    }
    #[test]
    fn registered_mock_model_infers_and_releases_lease() {
        let binder = binder();
        for _ in 0..3 {
            let result = binder
                .bind_and_execute(&request(RuntimeRouteSource::Gpu))
                .expect("queue inference");
            assert!(result.executed);
            assert!(result.inference.is_some());
        }
        assert!(binder.preparation_safe());
    }
    #[test]
    fn absent_device_or_wrong_input_never_claim_execution() {
        let binder = binder();
        assert!(
            !binder
                .bind_and_execute(&request(RuntimeRouteSource::Npu))
                .expect("decline")
                .executed
        );
        let mut bad = request(RuntimeRouteSource::Gpu);
        bad.inference = Some(InferenceRequest::default());
        assert!(
            !binder
                .bind_and_execute(&bad)
                .expect("dimension rejection")
                .executed
        );
        bad.inference = None;
        assert!(
            !binder
                .bind_and_execute(&bad)
                .expect("missing rejection")
                .executed
        );
    }
}
