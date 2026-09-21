use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;

/// Runtime 路由源。
///
/// 语义：
/// - `Native`：不改变应用默认执行路径（fallback 默认）。
/// - `Cpu`：CPU 执行。
/// - `Gpu`：GPU 执行。
/// - `Npu`：NPU 执行。
/// - `AiRuntime`：通过 AICore 的 `IAiRuntime` 执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteSource {
    Native,
    Cpu,
    Gpu,
    Npu,
    AiRuntime,
}

impl RuntimeRouteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Npu => "npu",
            Self::AiRuntime => "ai_runtime",
        }
    }

    /// 是否是"不改变应用"的路径。
    pub fn is_native(self) -> bool {
        self == Self::Native
    }
}

/// 一条 Runtime 路由决策。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeRoute {
    pub route_id: String,
    /// 决策对应的 workload。
    pub workload_type: WorkloadType,
    /// 选择的执行路径。
    pub source: RuntimeRouteSource,
    /// 是否真的应用了路由（false = 保持原生）。
    pub applied: bool,
    /// 决策原因。
    pub reason: String,
    /// 置信度，0.0 - 1.0。
    pub confidence: f32,
    /// 决策时间。
    pub decided_at_unix_secs: u64,
}

impl RuntimeRoute {
    pub fn native(workload: WorkloadType, reason: impl Into<String>) -> Self {
        Self {
            route_id: String::new(),
            workload_type: workload,
            source: RuntimeRouteSource::Native,
            applied: false,
            reason: reason.into(),
            confidence: 1.0,
            decided_at_unix_secs: 0,
        }
    }

    pub fn routed(
        workload: WorkloadType,
        source: RuntimeRouteSource,
        reason: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            route_id: String::new(),
            workload_type: workload,
            source,
            applied: true,
            reason: reason.into(),
            confidence: confidence.clamp(0.0, 1.0),
            decided_at_unix_secs: 0,
        }
    }

    pub fn is_native(&self) -> bool {
        self.source.is_native()
    }
}
