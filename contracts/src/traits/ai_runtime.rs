use crate::ai::runtime::{
    InferenceRequest, InferenceResponse, ModelHandle, ModelSpec, RuntimeCapability, RuntimeHealth,
};
use crate::errors::ContractResult;

/// AI 模型运行时抽象。
///
/// 实现可以是：
/// - `PureRustRuntime`（V3-F 提供）
/// - ONNX Runtime（V3-F.2 或更晚）
/// - llama.cpp（V3-F.2 或更晚）
/// - vendor runtime
///
/// 约束：
/// - AI Runtime **不直接调用 OS control API**。
/// - 出错必须返回 `ContractError`，不 panic。
/// - 生命周期：load → infer → unload。
pub trait IAiRuntime: Send + Sync {
    /// 加载模型。
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle>;

    /// 卸载模型。句柄无效时返回错误。
    fn unload(&self, handle: &ModelHandle) -> ContractResult<()>;

    /// 运行推理。
    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse>;

    /// Runtime 支持的能力。
    fn capabilities(&self) -> Vec<RuntimeCapability>;

    /// Runtime 健康状态。
    fn health(&self) -> RuntimeHealth;

    /// Runtime 名称。
    fn runtime_name(&self) -> &'static str;
}
