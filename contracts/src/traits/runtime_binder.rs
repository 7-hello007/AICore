use crate::ai::runtime::{InferenceRequest, InferenceResponse, ModelHandle};
use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::runtime::route::{RuntimeRoute, RuntimeRouteSource};

/// 绑定请求。
#[derive(Debug, Clone)]
pub struct BindingRequest {
    pub route: RuntimeRoute,
    pub context: ContextSnapshot,
    /// 如果上游已有 inference 请求，则一并传入。
    pub inference: Option<InferenceRequest>,
}

/// 绑定结果。
#[derive(Debug, Clone)]
pub struct BindingResult {
    pub route_id: String,
    pub source: RuntimeRouteSource,
    /// 是否真的执行了（false = 直接走 Native）。
    pub executed: bool,
    /// 执行输出（若 `executed == true`）。
    pub inference: Option<InferenceResponse>,
    /// 人类可读理由。
    pub reason: String,
    /// 实际耗时。
    pub latency_ms: u64,
}

/// Runtime Binder。
///
/// 职责：
/// - 接收 `RuntimeRoute`，绑定到具体 backend。
/// - 执行（若 route 非 Native）。
/// - 失败时返回 `Ok(BindingResult { executed: false, ... })`，由上层回退 Native。
///
/// **约束**：
/// - Binder 不直接调用 OS control API。
/// - Binder 只做 route → backend 的路由与推理。
/// - 执行失败必须 fallback 到 Native（`executed: false`）。
pub trait IRuntimeBinder: Send + Sync {
    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult>;

    /// 返回 binder 名称。
    fn name(&self) -> &'static str;

    /// 已加载的模型句柄（用于审计 / Dashboard）。
    fn loaded_models(&self) -> Vec<ModelHandle>;
}
