use crate::context::workload::WorkloadType;
use crate::errors::ContractResult;
use crate::telemetry::ComputerSnapshot;

/// Workload 观察结果。
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadObservation {
    pub workload_type: WorkloadType,
    /// 置信度，0.0 - 1.0。
    pub confidence: f32,
    /// 人类可读证据摘要。
    pub evidence: Vec<String>,
}

/// 观察者的语义类别。
///
/// 用于 Fusion 决定"规则优先"还是"ML 补充"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// 规则 / 确定性逻辑。
    Rule,
    /// 机器学习 / 统计模型。
    Ml,
    /// 应用显式声明（L1/L2）。
    Declared,
}

/// Workload 观察源。
///
/// 实现者可以是 L0 规则推断、ML 分类器、L1 声明、L2 原生 provider。
/// Core 通过此接口获取 workload；不关心具体 Tier。
pub trait IWorkloadSource: Send + Sync {
    fn observe(&self, snapshot: &ComputerSnapshot) -> ContractResult<WorkloadObservation>;

    /// 提供者名称，用于日志。
    fn source_name(&self) -> &'static str;

    /// 优先级；数值越大越优先。
    fn priority(&self) -> u32 {
        100
    }

    /// 语义类别。默认 Rule。
    fn kind(&self) -> SourceKind {
        SourceKind::Rule
    }

    /// 该源自我宣称的置信度上限。
    ///
    /// Fusion 用它判断"是否可以直接采纳"。
    fn confidence_ceiling(&self) -> f32 {
        1.0
    }
}
