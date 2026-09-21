pub mod input;
pub mod model_metadata;
pub mod runtime;
pub mod suggestion;
pub mod task;

pub use input::AiInput;
pub use model_metadata::{ModelMetadata, ModelRuntime, PrivacyClass};
pub use runtime::{
    InferenceLabel, InferenceRequest, InferenceResponse, ModelHandle, ModelKind, ModelSource,
    ModelSpec, RuntimeCapability, RuntimeHealth,
};
pub use suggestion::{AiSource, AiSuggestion, AiSuggestionPayload};
pub use task::AiTaskType;
