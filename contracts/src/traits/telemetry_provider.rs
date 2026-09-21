use crate::errors::ContractResult;
use crate::telemetry::{constraints::SamplingConstraints, ComputerSnapshot};

/// 所有平台适配器必须实现的遥测接口。
pub trait ITelemetryProvider: Send + Sync {
    /// 采集一次完整的系统状态快照。
    fn collect(&self) -> ContractResult<ComputerSnapshot>;

    /// 返回该 Provider 的名称。
    fn platform_name(&self) -> &'static str;

    /// 返回支持的能力列表。
    fn capabilities(&self) -> Vec<&'static str>;

    /// 返回采样约束。默认不限制。
    fn sampling_constraints(&self) -> SamplingConstraints {
        SamplingConstraints::default()
    }
}
