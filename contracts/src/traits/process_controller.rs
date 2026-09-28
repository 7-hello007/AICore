use crate::errors::ContractResult;
use crate::policy::priority::PolicyPriority;

/// CPU accompaniment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityMask {
    /// Logical core index list; index to `0..n-1`.
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

/// Process level control is abstract.
pub trait IProcessController: Send + Sync {
    fn set_priority(&self, pid: u32, priority: PolicyPriority) -> ContractResult<()>;
    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority>;

    /// Sets up the CPU relationship.
    /// Default returns CapabilityUnavailable so that unsupported platforms do not need to be realized.
    fn set_affinity(&self, _pid: u32, _mask: &AffinityMask) -> ContractResult<()> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "set_affinity not supported".to_string(),
        ))
    }

    /// Reads the CPU relationship. Default CapabilityUnavailable.
    fn get_affinity(&self, _pid: u32) -> ContractResult<AffinityMask> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "get_affinity not supported".to_string(),
        ))
    }

    fn capabilities(&self) -> Vec<&'static str>;
    fn platform_name(&self) -> &'static str;
}
