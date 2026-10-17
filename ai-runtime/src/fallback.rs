use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, InferenceResponse, ModelHandle, ModelSpec, RuntimeCapability,
        RuntimeHealth,
    },
    errors::{ContractError, ContractResult},
    traits::ai_runtime::IAiRuntime,
};

/// Runtime mixer with fallback.
///
/// Semantics:
/// - `primary` priority.
/// - Load/inference errors may select the fallback, with its own loaded handle.
/// - Unload attempts every owned backend and preserves failures for retry.
/// - Record the number of fallbacks for audit.
pub struct FallbackAiRuntime {
    primary: Arc<dyn IAiRuntime>,
    fallback: Arc<dyn IAiRuntime>,
    fallback_load_count: AtomicU64,
    fallback_infer_count: AtomicU64,
    models: Mutex<HashMap<String, Arc<Mutex<LoadedModel>>>>,
}

struct LoadedModel {
    spec: ModelSpec,
    primary: Option<ModelHandle>,
    fallback: Option<ModelHandle>,
    retiring: bool,
}

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

impl FallbackAiRuntime {
    pub fn new(primary: Arc<dyn IAiRuntime>, fallback: Arc<dyn IAiRuntime>) -> Self {
        Self {
            primary,
            fallback,
            fallback_load_count: AtomicU64::new(0),
            fallback_infer_count: AtomicU64::new(0),
            models: Mutex::new(HashMap::new()),
        }
    }

    pub fn primary_name(&self) -> &'static str {
        self.primary.runtime_name()
    }

    pub fn fallback_name(&self) -> &'static str {
        self.fallback.runtime_name()
    }

    pub fn fallback_load_count(&self) -> u64 {
        self.fallback_load_count.load(Ordering::SeqCst)
    }

    pub fn fallback_infer_count(&self) -> u64 {
        self.fallback_infer_count.load(Ordering::SeqCst)
    }

    fn model(&self, handle: &ModelHandle) -> ContractResult<Arc<Mutex<LoadedModel>>> {
        self.models
            .lock()
            .map_err(|_| {
                ContractError::ResourceUnavailable("fallback model registry poisoned".into())
            })?
            .get(&handle.handle_id)
            .cloned()
            .ok_or_else(|| {
                ContractError::InvalidRequest("unknown or unloaded fallback handle".into())
            })
    }
}

impl IAiRuntime for FallbackAiRuntime {
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle> {
        // Lock registry before acquiring a backend resource, so a poisoned
        // registry cannot strand a successfully loaded model.
        let mut models = self.models.lock().map_err(|_| {
            ContractError::ResourceUnavailable("fallback model registry poisoned".into())
        })?;
        let (primary, fallback) = match self.primary.load_model(spec) {
            Ok(h) => (Some(h), None),
            Err(primary_err) => {
                tracing::warn!(
                    component = "fallback_runtime",
                    event = "load_fallback",
                    primary = self.primary.runtime_name(),
                    fallback = self.fallback.runtime_name(),
                    error = %primary_err,
                    "primary load failed; trying fallback"
                );
                match self.fallback.load_model(spec) {
                    Ok(h) => {
                        self.fallback_load_count.fetch_add(1, Ordering::SeqCst);
                        (None, Some(h))
                    },
                    Err(fallback_err) => {
                        tracing::error!(
                            component = "fallback_runtime",
                            event = "load_all_failed",
                            primary_error = %primary_err,
                            fallback_error = %fallback_err,
                            "both primary and fallback failed"
                        );
                        return Err(ContractError::AiInferenceFailed(format!(
                            "primary: {}; fallback: {}",
                            primary_err, fallback_err
                        )));
                    },
                }
            },
        };
        let mut handle = primary
            .as_ref()
            .or(fallback.as_ref())
            .expect("successful load")
            .clone();
        handle.handle_id = format!("fallback-{}", NEXT_HANDLE.fetch_add(1, Ordering::SeqCst));
        models.insert(
            handle.handle_id.clone(),
            Arc::new(Mutex::new(LoadedModel {
                spec: spec.clone(),
                primary,
                fallback,
                retiring: false,
            })),
        );
        Ok(handle)
    }

    fn unload(&self, handle: &ModelHandle) -> ContractResult<()> {
        let model = self.model(handle)?;
        let mut state = model
            .lock()
            .map_err(|_| ContractError::ResourceUnavailable("fallback model poisoned".into()))?;
        state.retiring = true;
        let mut errors = Vec::new();
        if let Some(primary) = &state.primary {
            match self.primary.unload(primary) {
                Ok(()) => state.primary = None,
                Err(error) => errors.push(format!("primary: {error}")),
            }
        }
        if let Some(fallback) = &state.fallback {
            match self.fallback.unload(fallback) {
                Ok(()) => state.fallback = None,
                Err(error) => errors.push(format!("fallback: {error}")),
            }
        }
        if errors.is_empty() {
            drop(state);
            self.models
                .lock()
                .map_err(|_| {
                    ContractError::ResourceUnavailable("fallback registry poisoned".into())
                })?
                .remove(&handle.handle_id);
            Ok(())
        } else {
            Err(ContractError::ResourceUnavailable(errors.join("; ")))
        }
    }

    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse> {
        let model = self.model(handle)?;
        let mut state = model
            .lock()
            .map_err(|_| ContractError::ResourceUnavailable("fallback model poisoned".into()))?;
        if state.retiring {
            return Err(ContractError::InvalidRequest("model is unloading".into()));
        }
        let mut primary_error = None;
        if state.fallback.is_none() {
            if let Some(primary) = &state.primary {
                match self.primary.infer(primary, request) {
                    Ok(response) => return Ok(response),
                    Err(error) => primary_error = Some(error),
                }
            }
            match self.fallback.load_model(&state.spec) {
                Ok(fallback) => {
                    self.fallback_load_count.fetch_add(1, Ordering::SeqCst);
                    state.fallback = Some(fallback);
                },
                Err(error) => {
                    return Err(ContractError::AiInferenceFailed(format!(
                        "primary: {primary_error:?}; fallback load: {error}"
                    )))
                },
            }
        }
        match self
            .fallback
            .infer(state.fallback.as_ref().expect("loaded fallback"), request)
        {
            Ok(response) => {
                self.fallback_infer_count.fetch_add(1, Ordering::SeqCst);
                Ok(response)
            },
            Err(error) => Err(ContractError::AiInferenceFailed(format!(
                "primary: {primary_error:?}; fallback: {error}"
            ))),
        }
    }

    fn capabilities(&self) -> Vec<RuntimeCapability> {
        let mut caps = self.primary.capabilities();
        for c in self.fallback.capabilities() {
            if !caps.contains(&c) {
                caps.push(c);
            }
        }
        caps
    }

    fn health(&self) -> RuntimeHealth {
        // Report primary runtime health; otherwise report Fallback Health
        match self.primary.health() {
            RuntimeHealth::Healthy { loaded_models } if loaded_models > 0 => {
                RuntimeHealth::Healthy { loaded_models }
            },
            _ => self.fallback.health(),
        }
    }

    fn runtime_name(&self) -> &'static str {
        "fallback_runtime"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::ai::runtime::{ModelKind, ModelSource};
    use std::collections::HashSet;
    use std::sync::{atomic::AtomicBool, Mutex};

    struct Backend {
        name: &'static str,
        models: Mutex<HashSet<String>>,
        sequence: AtomicU64,
        fail_load: bool,
        fail_infer: bool,
        fail_unload_once: AtomicBool,
    }
    impl Backend {
        fn new(name: &'static str, fail_load: bool, fail_infer: bool) -> Arc<Self> {
            Arc::new(Self {
                name,
                models: Mutex::new(HashSet::new()),
                sequence: AtomicU64::new(0),
                fail_load,
                fail_infer,
                fail_unload_once: AtomicBool::new(false),
            })
        }
    }
    impl IAiRuntime for Backend {
        fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle> {
            if self.fail_load {
                return Err(ContractError::AiInferenceFailed("load fault".into()));
            }
            let id = format!(
                "{}-{}",
                self.name,
                self.sequence.fetch_add(1, Ordering::SeqCst)
            );
            self.models.lock().unwrap().insert(id.clone());
            Ok(ModelHandle {
                handle_id: id,
                model_name: spec.name.clone(),
                model_version: spec.version.clone(),
                loaded_at_unix_ms: 0,
            })
        }
        fn infer(
            &self,
            handle: &ModelHandle,
            _: &InferenceRequest,
        ) -> ContractResult<InferenceResponse> {
            if !self.models.lock().unwrap().contains(&handle.handle_id) {
                return Err(ContractError::InvalidRequest(
                    "foreign or unloaded handle".into(),
                ));
            }
            if self.fail_infer {
                return Err(ContractError::AiInferenceFailed("inference fault".into()));
            }
            Ok(InferenceResponse {
                labels: vec![],
                latency_ms: 0,
                model_version: self.name.into(),
            })
        }
        fn unload(&self, handle: &ModelHandle) -> ContractResult<()> {
            if self.fail_unload_once.swap(false, Ordering::SeqCst) {
                return Err(ContractError::ResourceUnavailable("unload fault".into()));
            }
            if self.models.lock().unwrap().remove(&handle.handle_id) {
                Ok(())
            } else {
                Err(ContractError::InvalidRequest(
                    "foreign or unloaded handle".into(),
                ))
            }
        }
        fn health(&self) -> RuntimeHealth {
            RuntimeHealth::Healthy {
                loaded_models: self.models.lock().unwrap().len(),
            }
        }
        fn capabilities(&self) -> Vec<RuntimeCapability> {
            vec![RuntimeCapability::FeatureInference]
        }
        fn runtime_name(&self) -> &'static str {
            self.name
        }
    }
    fn spec() -> ModelSpec {
        ModelSpec {
            name: "test".into(),
            version: "1".into(),
            kind: ModelKind::FeatureWeighted,
            source: ModelSource::Inline { json: "{}".into() },
            onnx_spec: None,
        }
    }

    #[test]
    fn p1_fallback_loads_and_uses_its_own_handle_after_primary_inference_failure() {
        for load_failure in [false, true] {
            let primary = Backend::new("primary", load_failure, true);
            let backup = Backend::new("backup", false, false);
            let runtime = FallbackAiRuntime::new(primary.clone(), backup.clone());
            let first = runtime.load_model(&spec()).unwrap();
            let second = runtime.load_model(&spec()).unwrap();
            for handle in [&first, &second] {
                assert_eq!(
                    runtime
                        .infer(handle, &InferenceRequest::default())
                        .unwrap()
                        .model_version,
                    "backup"
                );
                assert_eq!(
                    runtime
                        .infer(handle, &InferenceRequest::default())
                        .unwrap()
                        .model_version,
                    "backup"
                );
                runtime.unload(handle).unwrap();
                assert!(runtime.infer(handle, &InferenceRequest::default()).is_err());
            }
            assert_ne!(first.handle_id, second.handle_id);
            assert_eq!(
                primary.health(),
                RuntimeHealth::Healthy { loaded_models: 0 }
            );
            assert_eq!(backup.health(), RuntimeHealth::Healthy { loaded_models: 0 });
            assert_eq!(runtime.fallback_load_count(), 2);
            assert_eq!(runtime.fallback_infer_count(), 4);
        }
    }

    #[test]
    fn p1_partial_unload_releases_other_backend_and_can_retry() {
        let primary = Backend::new("primary", false, true);
        let backup = Backend::new("backup", false, false);
        let runtime = FallbackAiRuntime::new(primary.clone(), backup.clone());
        let handle = runtime.load_model(&spec()).unwrap();
        runtime
            .infer(&handle, &InferenceRequest::default())
            .unwrap();
        primary.fail_unload_once.store(true, Ordering::SeqCst);
        assert!(runtime.unload(&handle).is_err());
        assert!(
            runtime
                .infer(&handle, &InferenceRequest::default())
                .is_err(),
            "partial unload cannot reload or execute a retiring model"
        );
        assert_eq!(backup.health(), RuntimeHealth::Healthy { loaded_models: 0 });
        runtime.unload(&handle).unwrap();
        assert_eq!(
            primary.health(),
            RuntimeHealth::Healthy { loaded_models: 0 }
        );
        assert!(runtime.unload(&handle).is_err());
    }

    #[test]
    fn p1_primary_success_and_fallback_failures_preserve_owned_lifecycle() {
        for (primary_infer_fails, backup_load_fails, backup_infer_fails) in [
            (false, true, true),
            (true, true, false),
            (true, false, true),
        ] {
            let primary = Backend::new("primary", false, primary_infer_fails);
            let backup = Backend::new("backup", backup_load_fails, backup_infer_fails);
            let runtime = FallbackAiRuntime::new(primary.clone(), backup.clone());
            let handle = runtime.load_model(&spec()).unwrap();
            let response = runtime.infer(&handle, &InferenceRequest::default());
            if primary_infer_fails {
                assert!(response.is_err());
            } else {
                assert_eq!(response.unwrap().model_version, "primary");
                assert_eq!(runtime.fallback_load_count(), 0);
            }
            let mut invalid = handle.clone();
            invalid.handle_id.push_str("-foreign");
            assert!(runtime
                .infer(&invalid, &InferenceRequest::default())
                .is_err());
            runtime.unload(&handle).unwrap();
            assert_eq!(
                primary.health(),
                RuntimeHealth::Healthy { loaded_models: 0 }
            );
            assert_eq!(backup.health(), RuntimeHealth::Healthy { loaded_models: 0 });
        }
    }
}
