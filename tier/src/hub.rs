use std::sync::Arc;

use aicore_contracts::{
    tier::{capability::TierCapability, registration::TierRegistration},
    traits::{
        adaptation_provider::IAdaptationProvider, native_task_provider::INativeTaskProvider,
        task_source::ITaskSource, workload_source::IWorkloadSource,
    },
};

use crate::{
    capability_registry::{CapabilityRegistry, CapabilityRegistryError},
    registry::{RegistryError, TierRegistry},
};

/// TierHub is the sole entry point between Core and the tiers.
///
/// Composition:
/// - `TierRegistry` (metadata)
/// - Four `CapabilityRegistry` instances (capability implementations)
///
/// Core depends only on the hub; it neither holds tier instances nor matches TierId.
pub struct TierHub {
    registry: TierRegistry,
    workload_sources: CapabilityRegistry<dyn IWorkloadSource>,
    task_sources: CapabilityRegistry<dyn ITaskSource>,
    adaptation_providers: CapabilityRegistry<dyn IAdaptationProvider>,
    native_task_providers: CapabilityRegistry<dyn INativeTaskProvider>,
}

impl TierHub {
    pub fn new() -> Self {
        Self {
            registry: TierRegistry::new(),
            workload_sources: CapabilityRegistry::new(TierCapability::WorkloadObservation),
            task_sources: CapabilityRegistry::new(TierCapability::TaskMetadata),
            adaptation_providers: CapabilityRegistry::new(TierCapability::TransparentAdaptation),
            native_task_providers: CapabilityRegistry::new(TierCapability::NativeTaskControl),
        }
    }

    pub fn registry(&self) -> &TierRegistry {
        &self.registry
    }

    // === Metadata ===

    /// Register Tier metadata.
    pub fn register_tier(&self, reg: TierRegistration) -> Result<(), RegistryError> {
        self.registry.register(reg)
    }

    /// Unregister tier metadata and all of its capability implementations.
    ///
    /// Removing one tier must not affect the others.
    pub fn unregister_tier(
        &self,
        id: &aicore_contracts::tier::id::TierId,
    ) -> Result<bool, TierHubError> {
        let removed = self
            .registry
            .unregister(id)
            .map_err(|e| TierHubError::Registry(e.to_string()))?;

        // Remove capability implementations too, avoiding dangling providers.
        self.workload_sources
            .unregister(id)
            .map_err(|e| TierHubError::Capability(e.to_string()))?;
        self.task_sources
            .unregister(id)
            .map_err(|e| TierHubError::Capability(e.to_string()))?;
        self.adaptation_providers
            .unregister(id)
            .map_err(|e| TierHubError::Capability(e.to_string()))?;
        self.native_task_providers
            .unregister(id)
            .map_err(|e| TierHubError::Capability(e.to_string()))?;

        Ok(removed)
    }

    // === Capability registration ===

    pub fn register_workload_source(
        &self,
        tier_id: aicore_contracts::tier::id::TierId,
        priority: u32,
        provider: Arc<dyn IWorkloadSource>,
    ) -> Result<(), CapabilityRegistryError> {
        self.workload_sources.register(tier_id, priority, provider)
    }

    pub fn register_task_source(
        &self,
        tier_id: aicore_contracts::tier::id::TierId,
        priority: u32,
        provider: Arc<dyn ITaskSource>,
    ) -> Result<(), CapabilityRegistryError> {
        self.task_sources.register(tier_id, priority, provider)
    }

    pub fn register_adaptation_provider(
        &self,
        tier_id: aicore_contracts::tier::id::TierId,
        priority: u32,
        provider: Arc<dyn IAdaptationProvider>,
    ) -> Result<(), CapabilityRegistryError> {
        self.adaptation_providers
            .register(tier_id, priority, provider)
    }

    pub fn register_native_task_provider(
        &self,
        tier_id: aicore_contracts::tier::id::TierId,
        priority: u32,
        provider: Arc<dyn INativeTaskProvider>,
    ) -> Result<(), CapabilityRegistryError> {
        self.native_task_providers
            .register(tier_id, priority, provider)
    }

    // = = Capability queryCore Use) = =

    /// highest priority workload source; return None.
    pub fn best_workload_source(
        &self,
    ) -> Result<Option<Arc<dyn IWorkloadSource>>, CapabilityRegistryError> {
        self.workload_sources.best()
    }

    pub fn best_task_source(
        &self,
    ) -> Result<Option<Arc<dyn ITaskSource>>, CapabilityRegistryError> {
        self.task_sources.best()
    }

    pub fn best_adaptation_provider(
        &self,
    ) -> Result<Option<Arc<dyn IAdaptationProvider>>, CapabilityRegistryError> {
        self.adaptation_providers.best()
    }

    pub fn best_native_task_provider(
        &self,
    ) -> Result<Option<Arc<dyn INativeTaskProvider>>, CapabilityRegistryError> {
        self.native_task_providers.best()
    }

    /// All workloads, descend.
    pub fn all_workload_sources(
        &self,
    ) -> Result<Vec<Arc<dyn IWorkloadSource>>, CapabilityRegistryError> {
        self.workload_sources.all()
    }

    /// Is there a declaration of a certain ability.
    pub fn has_capability(&self, cap: TierCapability) -> Result<bool, TierHubError> {
        let regs = self
            .registry
            .find_by_capability(cap)
            .map_err(|e| TierHubError::Registry(e.to_string()))?;
        Ok(!regs.is_empty())
    }
}

impl Default for TierHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TierHubError {
    #[error("registry error: {0}")]
    Registry(String),
    #[error("capability registry error: {0}")]
    Capability(String),
}
