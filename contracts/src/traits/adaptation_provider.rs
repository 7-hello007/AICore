use crate::context::snapshot::ContextSnapshot;
use crate::errors::ContractResult;
use crate::policy::resource_policy::ResourcePolicy;
use crate::telemetry::ComputerSnapshot;

/// 透明适配建议。
#[derive(Debug, Clone)]
pub struct AdaptationHint {
    /// 建议覆盖的 policy（None 表示不覆盖）。
    pub policy_override: Option<ResourcePolicy>,
    /// 人类可读理由。
    pub reason: String,
    /// 置信度，0.0 - 1.0。
    pub confidence: f32,
}

/// 透明适配提供者。
///
/// L0 通过观察推断；L1 通过声明；L2 通过 native API。
/// Core 通过此接口获取建议；不关心具体 Tier。
pub trait IAdaptationProvider: Send + Sync {
    fn suggest(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<Option<AdaptationHint>>;

    fn source_name(&self) -> &'static str;

    fn priority(&self) -> u32 {
        100
    }
}
