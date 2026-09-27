use std::{path::Path, sync::Arc};

use aicore_ai_runtime::PureRustRuntime;
use aicore_contracts::{
    ai::{
        intent::{IntentDeployment, IntentProposal, IntentRequest},
        runtime::{
            InferenceRequest, ModelHandle, ModelKind, ModelSource, ModelSpec, TensorDType,
            TensorInput,
        },
    },
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    traits::{ai_runtime::IAiRuntime, intent_agent::IIntentAgent},
};
use serde::Deserialize;

/// Text adapter shared by JSON and ONNX classification runtimes. Vocabulary
/// and label mapping belong to the model, never Core. No host actuation.
pub struct RuntimeIntentAgent {
    runtime: Arc<dyn IAiRuntime>,
    handle: ModelHandle,
    vocabulary: Vec<String>,
    tensor_name: Option<String>,
}

impl RuntimeIntentAgent {
    pub fn load(
        runtime: Arc<dyn IAiRuntime>,
        spec: &ModelSpec,
        vocabulary: Vec<String>,
    ) -> ContractResult<Self> {
        if vocabulary.is_empty()
            || vocabulary.len() > 4096
            || vocabulary.iter().any(|word| {
                word.is_empty() || word.len() > 64 || word.chars().any(|c| !c.is_alphanumeric())
            })
            || vocabulary
                .iter()
                .enumerate()
                .any(|(i, word)| vocabulary[..i].contains(word))
        {
            return Err(ContractError::ContractInvalid(
                "invalid intent vocabulary".into(),
            ));
        }
        let tensor_name = match spec.kind {
            ModelKind::FeatureWeighted => None,
            ModelKind::Onnx => {
                let schemas = &spec
                    .onnx_spec
                    .as_ref()
                    .ok_or_else(|| ContractError::ContractInvalid("missing ONNX schema".into()))?
                    .input_schema;
                if schemas.len() != 1
                    || schemas[0].dtype != TensorDType::F32
                    || schemas[0].shape != vec![1, vocabulary.len() as i64]
                {
                    return Err(ContractError::ContractInvalid(
                        "intent ONNX expects a single [1,vocabulary] f32 tensor".into(),
                    ));
                }
                Some(schemas[0].name.clone())
            },
            _ => {
                return Err(ContractError::NotSupported(
                    "intent runtime model kind".into(),
                ))
            },
        };
        let handle = runtime.load_model(spec)?;
        Ok(Self {
            runtime,
            handle,
            vocabulary,
            tensor_name,
        })
    }

    pub fn local_json(path: &Path) -> ContractResult<Self> {
        #[derive(Deserialize)]
        struct Manifest {
            name: String,
            version: String,
            feature_dim: usize,
            vocabulary: Vec<String>,
        }
        let bytes =
            std::fs::read(path).map_err(|e| ContractError::AiInferenceFailed(e.to_string()))?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(ContractError::ContractInvalid(
                "intent model too large".into(),
            ));
        }
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|e| ContractError::ContractInvalid(e.to_string()))?;
        if manifest.feature_dim != manifest.vocabulary.len() {
            return Err(ContractError::ContractInvalid(
                "vocabulary dimension mismatch".into(),
            ));
        }
        let json =
            String::from_utf8(bytes).map_err(|e| ContractError::ContractInvalid(e.to_string()))?;
        Self::load(
            Arc::new(PureRustRuntime::new()),
            &ModelSpec {
                name: manifest.name,
                version: manifest.version,
                kind: ModelKind::FeatureWeighted,
                source: ModelSource::Inline { json },
                onnx_spec: None,
            },
            manifest.vocabulary,
        )
    }
}

impl Drop for RuntimeIntentAgent {
    fn drop(&mut self) {
        if let Err(error) = self.runtime.unload(&self.handle) {
            tracing::warn!(%error, "intent runtime unload failed");
        }
    }
}

impl IIntentAgent for RuntimeIntentAgent {
    fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal> {
        if request.text.trim().is_empty() || request.text.len() > 4096 {
            return Err(ContractError::ContractInvalid("invalid intent text".into()));
        }
        let text = request.text.to_lowercase();
        let words: Vec<_> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let features: Vec<_> = self
            .vocabulary
            .iter()
            .map(|word| {
                if words.contains(&word.as_str()) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        if features.iter().all(|value| *value == 0.0) {
            return Err(ContractError::AiInferenceFailed(
                "intent outside model vocabulary".into(),
            ));
        }
        let tensors = self
            .tensor_name
            .as_ref()
            .map(|name| {
                vec![TensorInput {
                    name: name.clone(),
                    shape: vec![1, features.len() as i64],
                    data: features.clone(),
                }]
            })
            .unwrap_or_default();
        let response = self
            .runtime
            .infer(&self.handle, &InferenceRequest { features, tensors })?;
        if response.labels.is_empty()
            || response
                .labels
                .iter()
                .any(|label| !label.score.is_finite() || !(0.0..=1.0).contains(&label.score))
        {
            return Err(ContractError::AiInferenceFailed(
                "intent output must be probabilities".into(),
            ));
        }
        let best = response
            .labels
            .iter()
            .max_by(|a, b| a.score.total_cmp(&b.score))
            .ok_or_else(|| ContractError::AiInferenceFailed("empty intent output".into()))?;
        let workload_type: WorkloadType =
            serde_json::from_value(serde_json::Value::String(best.label.clone()))
                .map_err(|_| ContractError::AiInferenceFailed("unknown intent label".into()))?;
        let proposal = IntentProposal {
            workload_type,
            confidence: best.score,
            reason: format!(
                "local runtime {} model {}",
                self.runtime.runtime_name(),
                self.handle.model_name
            ),
            expires_at_unix_ms: request.submitted_at_unix_ms.saturating_add(30_000),
        };
        crate::validate_intent_proposal(&proposal, request.submitted_at_unix_ms)?;
        Ok(proposal)
    }
    fn deployment(&self) -> IntentDeployment {
        IntentDeployment::Local
    }
    fn name(&self) -> &'static str {
        "runtime_intent_agent"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::application::id::ApplicationId;
    #[test]
    fn local_json_maps_tokens_and_rejects_unknown_input() {
        let agent =
            RuntimeIntentAgent::local_json(Path::new("../models/intent_classifier_v1.json"))
                .unwrap();
        let mut request = IntentRequest {
            application_id: ApplicationId::new("test.editor"),
            text: "compiling rust".into(),
            submitted_at_unix_ms: 10,
        };
        assert_eq!(
            agent.propose(&request).unwrap().workload_type,
            WorkloadType::Compilation
        );
        request.text = "unrelated vocabulary".into();
        assert!(agent.propose(&request).is_err());
    }
}
