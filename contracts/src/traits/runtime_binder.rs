use crate::ai::runtime::{InferenceRequest, InferenceResponse, ModelHandle};
use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::runtime::route::{RuntimeRoute, RuntimeRouteSource};

/// A binding request.
#[derive(Debug, Clone)]
pub struct BindingRequest {
    pub route: RuntimeRoute,
    pub context: ContextSnapshot,
    /// If there is an upstream request for access, it is entered together.
    pub inference: Option<InferenceRequest>,
}

/// Tie the results.
#[derive(Debug, Clone)]
pub struct BindingResult {
    pub route_id: String,
    pub source: RuntimeRouteSource,
    /// Whether the inference or the hardware operation was actually performed; only models are available, Native routers or not supported are false.
    pub executed: bool,
    /// Execute output (if `executed == true`).
    pub inference: Option<InferenceResponse>,
    /// Human readable reasons.
    pub reason: String,
    /// Actual time-consuming.
    pub latency_ms: u64,
}

/// Runtime Binder。
///
/// Duties:
/// - Receive `RuntimeRoute`, bind to specific back.
/// - Execute (if route is not native).
/// - Returns `Ok(BindingResult { executed: false, ... })` when failure, retreating from the upper layer of Native.
///
/// ** Binding**:
/// - Binder carries out hardware operations through the injection of Adapter executor, which does not contain the platform branch.
/// - Platform Binder may change the state of the device and cannot be placed in an irrevocable slow-line.
/// - Implementation failure must fallback to Native (`executed: false`).
pub trait IRuntimeBinder: Send + Sync {
    /// True only for isolated inference sessions with no host state setters.
    fn preparation_safe(&self) -> bool {
        false
    }

    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult>;

    /// returns the binder name.
    fn name(&self) -> &'static str;

    /// Loaded model handle (for audit / Dashboard).
    fn loaded_models(&self) -> Vec<ModelHandle>;
}
