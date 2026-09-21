use crate::errors::ContractResult;
use crate::policy::resource_policy::PowerMode;

/// 系统级电源策略抽象。
pub trait IPowerController: Send + Sync {
    /// 应用电源模式。返回实际生效的模式（平台可能降级）。
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode>;

    /// 读取当前电源模式。
    fn get_profile(&self) -> ContractResult<PowerMode>;

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}
