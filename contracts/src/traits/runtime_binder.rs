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
    /// 是否真的执行了推理或硬件操作；仅模型可用、Native 路由或不支持均为 false。
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
/// - Binder 通过注入的 Adapter executor 执行硬件操作，不包含平台分支。
/// - 平台 Binder 可能改变设备状态，不能放入不可取消的慢线程。
/// - 执行失败必须 fallback 到 Native（`executed: false`）。
pub trait IRuntimeBinder: Send + Sync {
    fn bind_and_execute(&self, request: &BindingRequest) -> ContractResult<BindingResult>;

    /// 返回 binder 名称。
    fn name(&self) -> &'static str;

    /// 已加载的模型句柄（用于审计 / Dashboard）。
    fn loaded_models(&self) -> Vec<ModelHandle>;
}
