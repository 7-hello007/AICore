use std::sync::{Arc, RwLock};

use aicore_contracts::tier::{capability::TierCapability, id::TierId};

/// Registration of individual competencies.
struct CapabilityEntry<T: ?Sized> {
    tier_id: TierId,
    priority: u32,
    provider: Arc<T>,
}

/// Universal competency registration form.
///
/// Each capability (WorkloadSource / TaskSource / ...) has an independent registry example.
/// Core used `best()`/`all()` query capability,** not TierId branch**.
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

    /// Register an implemented capability.
    ///
    /// - Same `tier_id` duplicate register overwrite.
    /// - Different `tier_id` can coexist; `priority` decides the order of choice.
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

    /// Write off a tier capability.
    pub fn unregister(&self, tier_id: &TierId) -> Result<bool, CapabilityRegistryError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        let before = entries.len();
        entries.retain(|e| &e.tier_id != tier_id);
        Ok(entries.len() < before)
    }

    /// Returns the highest priority provider. Empty registry returns `None`.
    pub fn best(&self) -> Result<Option<Arc<T>>, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.first().map(|e| e.provider.clone()))
    }

    /// returns all providers, in descending order.
    pub fn all(&self) -> Result<Vec<Arc<T>>, CapabilityRegistryError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| CapabilityRegistryError::LockPoisoned)?;
        Ok(entries.iter().map(|e| e.provider.clone()).collect())
    }

    /// Returns the list of TierId that corresponds to the provider.
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
