use crate::{
    BoundedIntentAgent, IntentAgentCoordinator, OllamaIntentAgent, OllamaIntentConfig,
    RuntimeIntentAgent,
};
use aicore_ai_runtime::onnx::{OnnxRuntimeBackend, PythonOnnxSessionFactory};
use aicore_contracts::{
    ai::runtime::{ModelKind, ModelSource, ModelSpec, OnnxModelSpec, TensorDType, TensorSchema},
    errors::{ContractError, ContractResult},
    traits::intent_agent::IIntentAgent,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

/// Backend selection lives outside Core. ONNX is explicit and never uses the
/// mock factory as a fallback. Project-local dependencies must already exist.
pub fn configured_intent_agent(project: &Path) -> ContractResult<Option<Arc<BoundedIntentAgent>>> {
    let backend = std::env::var("AICORE_INTENT_BACKEND").unwrap_or_else(|_| "ollama".into());
    let manifest = std::env::var_os("AICORE_INTENT_MODEL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| project.join("models/intent_classifier_v1.json"));
    let (agent, timeout): (Arc<dyn IIntentAgent>, Duration) = match backend.as_str() {
        "disabled" => return Ok(None),
        "json" => (
            Arc::new(RuntimeIntentAgent::local_json(&manifest)?),
            Duration::from_secs(5),
        ),
        "ollama" => {
            let config = OllamaIntentConfig::from_env()?;
            let timeout = config.timeout;
            (Arc::new(OllamaIntentAgent::new(config)?), timeout)
        },
        "ensemble" => {
            let config = OllamaIntentConfig::from_env()?;
            let deadline = config.timeout;
            let agents: Vec<Arc<dyn IIntentAgent>> = vec![
                Arc::new(RuntimeIntentAgent::local_json(&manifest)?),
                Arc::new(OllamaIntentAgent::new(config)?),
            ];
            (
                Arc::new(IntentAgentCoordinator::new(agents, deadline)?),
                deadline + Duration::from_secs(1),
            )
        },
        "onnx" => {
            let data: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&manifest)
                    .map_err(|e| ContractError::ConfigInvalid(e.to_string()))?,
            )
            .map_err(|e| ContractError::ConfigInvalid(e.to_string()))?;
            let vocabulary: Vec<String> = serde_json::from_value(data["vocabulary"].clone())
                .map_err(|e| ContractError::ConfigInvalid(e.to_string()))?;
            let labels = data["labels"]
                .as_array()
                .ok_or_else(|| ContractError::ConfigInvalid("intent labels missing".into()))?
                .len();
            let python = std::env::var_os("AICORE_ONNX_PYTHON")
                .map(PathBuf::from)
                .ok_or_else(|| {
                    ContractError::ConfigInvalid("AICORE_ONNX_PYTHON is required".into())
                })?;
            let timeout = Duration::from_secs(10);
            let factory = PythonOnnxSessionFactory {
                python,
                runner: project.join("scripts/models/onnx_intent_runner.py"),
                packages: project.join("env/onnx-packages"),
                timeout,
            };
            let path = std::env::var_os("AICORE_ONNX_MODEL_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| manifest.with_extension("onnx"));
            let spec = ModelSpec {
                name: "intent_onnx".into(),
                version: "1".into(),
                kind: ModelKind::Onnx,
                source: ModelSource::File {
                    path: path.to_string_lossy().into_owned(),
                },
                onnx_spec: Some(OnnxModelSpec {
                    input_schema: vec![TensorSchema {
                        name: "tokens".into(),
                        shape: vec![1, vocabulary.len() as i64],
                        dtype: TensorDType::F32,
                    }],
                    output_schema: vec![TensorSchema {
                        name: "probabilities".into(),
                        shape: vec![1, labels as i64],
                        dtype: TensorDType::F32,
                    }],
                    preferred_provider: Some("CPUExecutionProvider".into()),
                }),
            };
            let runtime = Arc::new(
                OnnxRuntimeBackend::new(Arc::new(factory))
                    .with_timeout(timeout + Duration::from_secs(1)),
            );
            (
                Arc::new(RuntimeIntentAgent::load(runtime, &spec, vocabulary)?),
                timeout + Duration::from_secs(2),
            )
        },
        _ => {
            return Err(ContractError::ConfigInvalid(format!(
                "unsupported intent backend: {backend}"
            )))
        },
    };
    Ok(Some(Arc::new(BoundedIntentAgent::new(agent, timeout)?)))
}
