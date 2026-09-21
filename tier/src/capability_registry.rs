use std::sync::{Arc, RwLock};

use aicore_contracts::tier::{capability::TierCapability, id::TierId};

/// 单个能力实现的注册项。
struct CapabilityEntry<T: ?Sized> {
    tier_id: TierId,
    priority: u32,
    provider: Arc<T>,
}

/// 通用能力注册表。
///
/// 每种能力（WorkloadSource / TaskSource / ...）有一个独立的 registry 实例。
/// Core 通过 `best()` / `all()` 查询能力，**不通过 TierId 分支**。
pub struct CapabilityRegistry<T: ?Sized> {
    capability: TierCapability,
    entries: RwLock<Vec<CapabilityEntry<T>>>,
}

impl<T: ?Sized + Send + Sync + 'static> CapabilityRegistry<T> {
    pub fn new(capability: TierCapability) -> Self {
        Self {
            capability,
            entries: RwLock::new(Vec::new()),
        }
    }

    pub fn capability(&self) -> TierCapability {
        self.capability
    }

    /// 注册一个能力实现。
    ///
    /// - 相同 `tier_id` 重复注册 → 覆盖。
    /// - 不同 `tier_id` 可以共存；`priority` 决定选择顺序。
    pub fn register(
        &self,
        tier_id: TierId,
        priority: u32,
        provider: Arc<T>,
    ) -> Result<(), CapabilityRegistryError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;

        entries.retain(|e| e.tier_id != tier_id);
        entries.push(CapabilityEntry {
            tier_id,
            priority,
            provider,
        });
        entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(())
    }

    /// 注销某个 tier 的能力。
    pub fn unregister(&self, tier_id: &TierId) -> Result<bool, CapabilityRegistryError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        let before = entries.len();
        entries.retain(|e| &e.tier_id != tier_id);
        Ok(entries.len() < before)
    }

    /// 返回优先级最高的 provider。空 registry 返回 `None`。
    pub fn best(&self) -> Result<Option<Arc<T>>, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.first().map(|e| e.provider.clone()))
    }

    /// 返回所有 provider，按 priority 降序。
    pub fn all(&self) -> Result<Vec<Arc<T>>, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.iter().map(|e| e.provider.clone()).collect())
    }

    /// 返回 provider 对应的 TierId 列表。
    pub fn tier_ids(&self) -> Result<Vec<TierId>, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.iter().map(|e| e.tier_id.clone()).collect())
    }

    pub fn count(&self) -> Result<usize, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.len())
    }

    pub fn is_empty(&self) -> Result<bool, CapabilityRegistryError> {
        Ok(self.count()? == 0)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CapabilityRegistryError {
    #[error("capability registry lock poisoned")]
    LockPoisoned,
}
