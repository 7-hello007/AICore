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

/// Tier Hub：Core 与 Tier 之间的唯一入口。
///
/// 组合：
/// - `TierRegistry`（元数据）
/// - 4 个 `CapabilityRegistry`（能力实现）
///
/// Core **只依赖 Hub**；不直接持有任何 Tier 实例，不匹配 TierId。
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

    // === 元数据 ===

    /// 注册 Tier 元数据。不注册能力实现。
    pub fn register_tier(&self, reg: TierRegistration) -> Result<(), RegistryError> {
        self.registry.register(reg)
    }

    /// 注销 Tier 元数据 **以及** 该 Tier 的所有能力实现。
    ///
    /// 这是"删除 Tier 不影响其他 Tier"的关键实现。
    pub fn unregister_tier(
        &self,
        id: &aicore_contracts::tier::id::TierId,
    ) -> Result<bool, TierHubError> {
        let removed = self
            .registry
            .unregister(id)
            .map_err(|e| TierHubError::Registry(e.to_string()))?;

        // 能力实现也要一起注销，否则会留下悬空 provider。
        let _ = self.workload_sources.unregister(id);
        let _ = self.task_sources.unregister(id);
        let _ = self.adaptation_providers.unregister(id);
        let _ = self.native_task_providers.unregister(id);

        Ok(removed)
    }

    // === 能力注册 ===

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

    // === 能力查询（Core 使用） ===

    /// 最高优先级的 workload source；空时返回 None。
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

    /// 所有 workload sources，按 priority 降序。
    pub fn all_workload_sources(
        &self,
    ) -> Result<Vec<Arc<dyn IWorkloadSource>>, CapabilityRegistryError> {
        self.workload_sources.all()
    }

    /// 是否声明了某能力。
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
