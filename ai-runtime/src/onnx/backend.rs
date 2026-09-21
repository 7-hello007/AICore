use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, InferenceResponse, ModelHandle, ModelKind, ModelSource, ModelSpec,
        RuntimeCapability, RuntimeHealth,
    },
    errors::{ContractError, ContractResult},
    traits::ai_runtime::IAiRuntime,
};

use super::metrics::InferenceMetrics;
use super::session::{IOnnxSession, IOnnxSessionFactory};

/// 已加载的 ONNX 模型。
struct LoadedOnnxModel {
    handle: ModelHandle,
    session: Arc<dyn IOnnxSession>,
}

/// ONNX Runtime 后端。
///
/// 职责：
/// - 从 `ModelSpec`（`kind == Onnx`）加载模型。
/// - 校验 `onnx_spec`。
/// - 通过 `IOnnxSessionFactory` 创建 session。
/// - 推理带超时。
/// - 失败不 panic；返回 `ContractError`。
/// - 记录 metrics。
pub struct OnnxRuntimeBackend {
    factory: Arc<dyn IOnnxSessionFactory>,
    models: Mutex<Vec<LoadedOnnxModel>>,
    metrics: Arc<InferenceMetrics>,
    timeout: Duration,
    next_seq: AtomicU64,
}

impl OnnxRuntimeBackend {
    pub fn new(factory: Arc<dyn IOnnxSessionFactory>) -> Self {
        Self {
            factory,
            models: Mutex::new(Vec::new()),
            metrics: Arc::new(InferenceMetrics::new()),
            timeout: Duration::from_secs(5),
            next_seq: AtomicU64::new(1),
        }
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn metrics(&self) -> &Arc<InferenceMetrics> {
        &self.metrics
    }

    pub fn loaded_count(&self) -> usize {
        self.models.lock().map(|m| m.len()).unwrap_or(0)
    }

    fn read_source(source: &ModelSource) -> ContractResult<PathBuf> {
        match source {
            ModelSource::File { path } => Ok(PathBuf::from(path)),
            ModelSource::Inline { .. } => Err(ContractError::NotSupported(
                "ONNX requires ModelSource::File".to_string(),
            )),
        }
    }
}

impl IAiRuntime for OnnxRuntimeBackend {
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle> {
        if spec.kind != ModelKind::Onnx {
            return Err(ContractError::NotSupported(format!(
                "OnnxRuntimeBackend requires ModelKind::Onnx, got {:?}",
                spec.kind
            )));
        }

        spec.validate()?;
        let onnx_spec = spec
            .onnx_spec
            .as_ref()
            .ok_or_else(|| ContractError::InvalidRequest("missing onnx_spec".to_string()))?;

        let path = Self::read_source(&spec.source)?;

        let session = self
            .factory
            .create_session(onnx_spec, &path)
            .map_err(|e| ContractError::ResourceUnavailable(format!("create session: {}", e)))?;

        // 提前取 provider 名称，避免 session 被 move 后再借用
        let provider_name = session.provider_name();

        let now = now_unix_secs();
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let handle = ModelHandle {
            handle_id: format!("onnx-{}-{}-{}", spec.name, now, seq),
            model_name: spec.name.clone(),
            model_version: spec.version.clone(),
            loaded_at_unix_ms: now,
        };

        let mut models = self
            .models
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("onnx backend lock poisoned".into()))?;
        models.push(LoadedOnnxModel {
            handle: handle.clone(),
            session,
        });

        tracing::info!(
            component = "onnx_runtime_backend",
            event = "model_loaded",
            handle_id = %handle.handle_id,
            model = %handle.model_name,
            provider = provider_name,
            "onnx model loaded"
        );

        Ok(handle)
    }

    fn unload(&self, handle: &ModelHandle) -> ContractResult<()> {
        let mut models = self
            .models
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("onnx backend lock poisoned".into()))?;
        let before = models.len();
        models.retain(|m| m.handle.handle_id != handle.handle_id);
        if models.len() == before {
            return Err(ContractError::ResourceUnavailable(format!(
                "onnx model not loaded: {}",
                handle.handle_id
            )));
        }
        Ok(())
    }

    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse> {
        let start = Instant::now();

        // 1. 拿 session（短暂持锁）
        let session: Arc<dyn IOnnxSession> = {
            let models = self.models.lock().map_err(|_| {
                ContractError::AdapterUnavailable("onnx backend lock poisoned".into())
            })?;
            models
                .iter()
                .find(|m| m.handle.handle_id == handle.handle_id)
                .map(|m| m.session.clone())
                .ok_or_else(|| {
                    ContractError::ResourceUnavailable(format!(
                        "onnx model not loaded: {}",
                        handle.handle_id
                    ))
                })?
        };

        // 2. 校验张量输入非空
        if request.tensors.is_empty() {
            self.metrics
                .record_failure(start.elapsed().as_millis() as u64);
            return Err(ContractError::InvalidRequest(
                "ONNX inference requires tensors".to_string(),
            ));
        }

        // 3. 在独立线程执行并由调用方超时边界控制等待时间。
        // IOnnxSession 没有统一的取消接口，因此超时后无法强制终止第三方 runtime 的内部计算；
        // 但 AICore 不再阻塞当前 control loop。具体 provider 可在自身实现中提供更强取消语义。
        let tensors = request.tensors.clone();
        let session_for_run = session.clone();
        let timeout = self.timeout;
        let (tx, rx) = std::sync::mpsc::channel::<
            Result<Vec<aicore_contracts::ai::runtime::InferenceLabel>, String>,
        >();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session_for_run.run(&tensors)
            }))
            .unwrap_or_else(|_| Err("onnx session panicked".to_string()));
            let _ = tx.send(result);
        });

        let result = rx.recv_timeout(timeout);
        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.metrics.record_timeout(latency_ms);
                Err(ContractError::AiTimeout(format!(
                    "inference exceeded {} ms",
                    timeout.as_millis()
                )))
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                self.metrics.record_failure(latency_ms);
                Err(ContractError::AiInferenceFailed(
                    "onnx worker disconnected".to_string(),
                ))
            },
            Ok(Ok(labels)) => {
                self.metrics.record_success(latency_ms);
                Ok(InferenceResponse {
                    labels,
                    latency_ms,
                    model_version: handle.model_version.clone(),
                })
            },
            Ok(Err(e)) => {
                self.metrics.record_failure(latency_ms);
                Err(ContractError::AiInferenceFailed(e))
            },
        }
    }

    fn capabilities(&self) -> Vec<RuntimeCapability> {
        vec![
            RuntimeCapability::LoadFromFile,
            RuntimeCapability::OnnxInference,
        ]
    }

    fn health(&self) -> RuntimeHealth {
        match self.models.lock() {
            Ok(models) => RuntimeHealth::Healthy {
                loaded_models: models.len(),
            },
            Err(_) => RuntimeHealth::Degraded {
                reason: "onnx backend lock poisoned".to_string(),
            },
        }
    }

    fn runtime_name(&self) -> &'static str {
        "onnx_runtime_backend"
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
