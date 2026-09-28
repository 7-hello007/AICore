use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::errors::AicoreError;
use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};

/// Tier registration form.
///
/// ** The Tier instance is not saved** - Only registered metadata are kept.
/// Capabilities are registered through an independent `CapabilityRegistry` (next).
pub struct TierRegistry {
    registrations: RwLock<HashMap<TierId, TierRegistration>>,
}

impl TierRegistry {
    pub fn new() -> Self {
        Self {
            registrations: RwLock::new(HashMap::new()),
        }
    }

    /// Register a Tier.
    ///
    /// - Validate metadata validity.
    /// - Same id duplicate registration rejected (avoid accidental coverage).
    pub fn register(&self, reg: TierRegistration) -> Result<(), RegistryError> {
        reg.validate().map_err(RegistryError::InvalidRegistration)?;

        let mut regs = self
            .registrations
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        if regs.contains_key(&reg.id) {
            return Err(RegistryError::DuplicateId(reg.id.0.clone()));
        }

        regs.insert(reg.id.clone(), reg);
        Ok(())
    }

    /// Write off one Tier.
    ///
    /// Returns whether removal is successful.
    pub fn unregister(&self, id: &TierId) -> Result<bool, RegistryError> {
        let mut regs = self
            .registrations
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.remove(id).is_some())
    }

    /// Query single Tier.
    pub fn get(&self, id: &TierId) -> Result<Option<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.get(id).cloned())
    }

    /// List all of Tiers.
    pub fn list(&self) -> Result<Vec<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<TierRegistration> = regs.values().cloned().collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Query by capability.
    pub fn find_by_capability(
        &self,
        cap: TierCapability,
    ) -> Result<Vec<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<TierRegistration> = regs
            .values()
            .filter(|r| r.has_capability(cap))
            .cloned()
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Press level query.
    pub fn find_by_level(&self, level: TierLevel) -> Result<Vec<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<TierRegistration> = regs
            .values()
            .filter(|r| r.level == level)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Statistics.
    pub fn count(&self) -> Result<usize, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.len())
    }

    /// Is it empty?
    pub fn is_empty(&self) -> Result<bool, RegistryError> {
        Ok(self.count()? == 0)
    }
}

impl Default for TierRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegistryError {
    #[error("registry lock poisoned")]
    LockPoisoned,
    #[error("invalid registration: {0}")]
    InvalidRegistration(#[from] AicoreError),
    #[error("duplicate tier id: {0}")]
    DuplicateId(String),
}
