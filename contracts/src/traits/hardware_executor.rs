use crate::errors::ContractResult;

/// 硬件执行提示。
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareExecutionHint {
    /// 工作负载偏好，例如 "graphics" / "compute" / "auto"。
    WorkloadPreference(String),
    /// 最大功耗瓦特（尽力而为）。
    MaxPowerWatts(f32),
    /// 仅图形模式。
    GraphicsMode,
    /// 仅计算模式。
    ComputeMode,
    /// 自动模式。
    AutoMode,
}

/// 硬件执行状态。
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareExecutionStatus {
    /// 已应用。
    Applied,
    /// 已在目标状态（无变化）。
    AlreadyInState,
    /// 被跳过（例如能力可用但用户禁用）。
    Skipped { reason: String },
    /// 平台不支持。
    NotSupported { reason: String },
    /// 执行失败。
    Failed { reason: String },
}

impl HardwareExecutionStatus {
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied | Self::AlreadyInState)
    }
}

/// 硬件执行请求。
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareExecutionRequest {
    pub device_id: String,
    pub hint: HardwareExecutionHint,
}

/// 硬件执行结果。
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareExecutionResult {
    pub device_id: String,
    pub status: HardwareExecutionStatus,
    pub latency_ms: u64,
}

/// 硬件执行器。
///
/// 职责：
/// - 把 `HardwareExecutionHint` 应用到具体设备。
/// - 失败不 panic；通过 `HardwareExecutionStatus` 表达。
/// - 不直接调用 Executor；由 Binder 调用。
///
/// 平台：
/// - Linux：尽力而为、capability-gated 的 sysfs / vendor-runtime 写入。
/// - Windows：尽力而为、vendor-specific 的控制（例如 NVIDIA），其他路径返回 `NotSupported`。
///
/// 重要：`NotSupported` 是合法的能力缺失，不得伪造成执行成功。
pub trait IHardwareExecutor: Send + Sync {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult>;

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult>;

    fn executor_name(&self) -> &'static str;
}
