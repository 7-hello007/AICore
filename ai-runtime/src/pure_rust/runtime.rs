use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    ai::runtime::{
        InferenceLabel, InferenceRequest, InferenceResponse, ModelHandle, ModelKind, ModelSource,
        ModelSpec, RuntimeCapability, RuntimeHealth,
    },
    errors::{ContractError, ContractResult},
    traits::ai_runtime::IAiRuntime,
};

use super::model::ModelWeights;

struct LoadedModel {
    handle: ModelHandle,
    weights: ModelWeights,
}

/// 纯 Rust 模型运行时。
///
/// - 从 JSON 文件或内联 JSON 加载权重。
/// - 支持多个模型同时加载。
/// - 提供 load / unload / infer / health 完整生命周期。
/// - 无外部依赖，无 C++ 工具链。
pub struct PureRustRuntime {
    models: Mutex<Vec<LoadedModel>>,
    /// handle 序号，保证唯一。
    next_seq: AtomicU64,
}

impl PureRustRuntime {
    pub fn new() -> Self {
        Self {
            models: Mutex::new(Vec::new()),
            next_seq: AtomicU64::new(1),
        }
    }

    pub fn loaded_model_count(&self) -> usize {
        self.models.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn loaded_models(&self) -> Vec<ModelHandle> {
        self.models
            .lock()
            .map(|models| models.iter().map(|m| m.handle.clone()).collect())
            .unwrap_or_default()
    }

    fn read_source(source: &ModelSource) -> ContractResult<String> {
        match source {
            ModelSource::File { path } => std::fs::read_to_string(path).map_err(|e| {
                ContractError::ResourceUnavailable(format!("read model {}: {}", path, e))
            }),
            ModelSource::Inline { json } => Ok(json.clone()),
        }
    }
}

impl Default for PureRustRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl IAiRuntime for PureRustRuntime {
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle> {
        if spec.kind != ModelKind::FeatureWeighted {
            return Err(ContractError::NotSupported(format!(
                "unsupported model kind: {:?}",
                spec.kind
            )));
        }

        let json = Self::read_source(&spec.source)?;
        let weights: ModelWeights = serde_json::from_str(&json)
            .map_err(|e| ContractError::InvalidRequest(format!("parse model: {}", e)))?;
        weights
            .validate()
            .map_err(|e| ContractError::InvalidRequest(format!("invalid model: {}", e)))?;

        let now = now_unix_ms();
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let handle = ModelHandle {
            handle_id: format!("model-{}-{}-{}", weights.name, now, seq),
            model_name: weights.name.clone(),
            model_version: weights.version.clone(),
            loaded_at_unix_ms: now,
        };

        let mut models = self
            .models
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("runtime lock poisoned".into()))?;

        models.push(LoadedModel {
            handle: handle.clone(),
            weights,
        });

        tracing::info!(
            component = "pure_rust_runtime",
            event = "model_loaded",
            handle_id = %handle.handle_id,
            model = %handle.model_name,
            version = %handle.model_version,
            "model loaded"
        );

        Ok(handle)
    }

    fn unload(&self, handle: &ModelHandle) -> ContractResult<()> {
        let mut models = self
            .models
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("runtime lock poisoned".into()))?;
        let before = models.len();
        models.retain(|m| m.handle.handle_id != handle.handle_id);
        if models.len() == before {
            return Err(ContractError::ResourceUnavailable(format!(
                "model handle not loaded: {}",
                handle.handle_id
            )));
        }

        tracing::info!(
            component = "pure_rust_runtime",
            event = "model_unloaded",
            handle_id = %handle.handle_id,
            "model unloaded"
        );

        Ok(())
    }

    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse> {
        let start = Instant::now();

        let models = self
            .models
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("runtime lock poisoned".into()))?;

        let model = models
            .iter()
            .find(|m| m.handle.handle_id == handle.handle_id)
            .ok_or_else(|| {
                ContractError::ResourceUnavailable(format!(
                    "model handle not loaded: {}",
                    handle.handle_id
                ))
            })?;

        let scored = model
            .weights
            .infer(&request.features)
            .map_err(ContractError::InvalidRequest)?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let model_version = model.weights.version.clone();

        let labels: Vec<InferenceLabel> = scored
            .into_iter()
            .map(|(label, score)| InferenceLabel { label, score })
            .collect();

        Ok(InferenceResponse {
            labels,
            latency_ms,
            model_version,
        })
    }

    fn capabilities(&self) -> Vec<RuntimeCapability> {
        vec![
            RuntimeCapability::LoadFromFile,
            RuntimeCapability::LoadFromMemory,
            RuntimeCapability::FeatureInference,
        ]
    }

    fn health(&self) -> RuntimeHealth {
        match self.models.lock() {
            Ok(models) => RuntimeHealth::Healthy {
                loaded_models: models.len(),
            },
            Err(_) => RuntimeHealth::Degraded {
                reason: "runtime lock poisoned".to_string(),
            },
        }
    }

    fn runtime_name(&self) -> &'static str {
        "pure_rust_runtime"
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
