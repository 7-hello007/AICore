use crate::errors::ContractResult;
use crate::policy::priority::PolicyPriority;

/// CPU accompaniment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityMask {
    /// Logical core index list; index to `0..n-1`.
    pub cores: Vec<u32>,
}

/// Opaque native priority, preserving values that PolicyPriority cannot express.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessPrioritySnapshot {
    pub platform: String,
    pub pid: u32,
    pub state_id: String,
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

    fn capture_priority(&self, pid: u32) -> ContractResult<ProcessPrioritySnapshot> {
        Ok(ProcessPrioritySnapshot {
            platform: self.platform_name().into(),
            pid,
            state_id: serde_json::to_string(&self.get_priority(pid)?)
                .map_err(|e| crate::errors::ContractError::ContractInvalid(e.to_string()))?,
        })
    }

    fn restore_priority(&self, state: &ProcessPrioritySnapshot) -> ContractResult<()> {
        if state.platform != self.platform_name() {
            return Err(crate::errors::ContractError::ContractInvalid(
                "process snapshot adapter mismatch".into(),
            ));
        }
        let priority = serde_json::from_str(&state.state_id)
            .map_err(|e| crate::errors::ContractError::ContractInvalid(e.to_string()))?;
        self.set_priority(state.pid, priority)?;
        if self.capture_priority(state.pid)? != *state {
            return Err(crate::errors::ContractError::AdapterUnavailable(
                "priority restoration readback differs".into(),
            ));
        }
        Ok(())
    }

    /// Native adapters bind validation and mutation to the captured process lifetime.
    fn set_priority_captured(
        &self,
        state: &ProcessPrioritySnapshot,
        priority: PolicyPriority,
    ) -> ContractResult<()> {
        self.set_priority(state.pid, priority)
    }

    fn set_affinity_captured(
        &self,
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        self.set_affinity(state.pid, mask)
    }

    /// Sets up the CPU relationship.
    /// Default returns CapabilityUnavailable so that unsupported platforms do not need to be realized.
    fn set_affinity(&self, _pid: u32, _mask: &AffinityMask) -> ContractResult<()> {
        Err(crate::errors::ContractError::CapabilityUnavailable(
            "set_affinity not supported".to_string(),
        ))
    }

    /// Restore affinity using the identity captured before the transaction.
    /// Native adapters must also validate the process lifetime.
    fn restore_affinity(
        &self,
        state: &ProcessPrioritySnapshot,
        mask: &AffinityMask,
    ) -> ContractResult<()> {
        if state.platform != self.platform_name() {
            return Err(crate::errors::ContractError::ContractInvalid(
                "process snapshot adapter mismatch".into(),
            ));
        }
        self.set_affinity(state.pid, mask)?;
        let mut observed = self.get_affinity(state.pid)?.cores;
        let mut expected = mask.cores.clone();
        observed.sort_unstable();
        expected.sort_unstable();
        if observed != expected {
            return Err(crate::errors::ContractError::AdapterUnavailable(
                "affinity restoration readback differs".into(),
            ));
        }
        Ok(())
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
