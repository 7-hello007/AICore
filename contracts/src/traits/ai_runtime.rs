use crate::ai::runtime::{
    InferenceRequest, InferenceResponse, ModelHandle, ModelSpec, RuntimeCapability, RuntimeHealth,
};
use crate::errors::ContractResult;

/// AI models run in abstracto.
///
/// This can be implemented by:
/// - `PureRustRuntime` (provided by V3-F)
/// - ONNX Runtime (V3-F.2 or later)
/// -llama.cpp (V3-F.2 or later)
/// - vendor runtime
///
/// Binding:
/// - AI Runtime** does not directly call OS control API**.
/// - Error must return `ContractError`, not Panic.
/// - Life cycle: load → infer unload.
pub trait IAiRuntime: Send + Sync {
    /// Load model.
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle>;

    /// Unmounts the model. Returns an error when the handle is invalid.
    fn unload(&self, handle: &ModelHandle) -> ContractResult<()>;

    /// Run the inference.
    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse>;

    /// The capability of Runtime support.
    fn capabilities(&self) -> Vec<RuntimeCapability>;

    /// Runtime state of health.
    fn health(&self) -> RuntimeHealth;

    /// Runtime name.
    fn runtime_name(&self) -> &'static str;
}
