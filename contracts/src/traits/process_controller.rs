use crate::errors::ContractResult;
use crate::policy::priority::PolicyPriority;

/// CPU 亲和性配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityMask {
    /// 逻辑核心索引列表；索引对应 `0..n-1`。
    pub cores: Vec<u32>,
}

impl AffinityMask {
    pub fn new(cores: Vec<u32>) -> Self {
        Self { cores }
    }

    pub fn is_empty(&self) -> bool {
        self.cores.is_empty()
    }
}

/// 进程级控制抽象。
pub trait IProcessController: Send + Sync {
    fn set_priority(&self, pid: u32, priority: PolicyPriority) -> ContractResult<()>;
    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority>;

    /// 设置进程 CPU 亲和性。
    /// 默认返回 CapabilityUnavailable，让不支持的平台无需实现。
    fn set_affinity(&self, _pid: u32, _mask: &AffinityMask) -> ContractResult<()> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "set_affinity not supported".to_string(),
        ))
    }

    /// 读取进程 CPU 亲和性。默认 CapabilityUnavailable。
    fn get_affinity(&self, _pid: u32) -> ContractResult<AffinityMask> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "get_affinity not supported".to_string(),
        ))
    }

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}
