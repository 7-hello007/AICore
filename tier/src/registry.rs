use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::errors::AicoreError;
use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};

/// Tier 注册表。
///
/// **不保存 Tier 实例**——只保存注册元数据。
/// 能力实现通过独立的 `CapabilityRegistry`（下一步）注册。
pub struct TierRegistry {
    registrations: RwLock<HashMap<TierId, TierRegistration>>,
}

impl TierRegistry {
    pub fn new() -> Self {
        Self {
            registrations: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个 Tier。
    ///
    /// - 校验 metadata 合法性。
    /// - 同 id 重复注册 → 拒绝（避免意外覆盖）。
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

    /// 注销一个 Tier。
    ///
    /// 返回是否成功移除。
    pub fn unregister(&self, id: &TierId) -> Result<bool, RegistryError> {
        let mut regs = self
            .registrations
            .write()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.remove(id).is_some())
    }

    /// 查询单个 Tier。
    pub fn get(&self, id: &TierId) -> Result<Option<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.get(id).cloned())
    }

    /// 列出所有 Tier。
    pub fn list(&self) -> Result<Vec<TierRegistration>, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let mut out: Vec<TierRegistration> = regs.values().cloned().collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// 按能力查询。
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

    /// 按 level 查询。
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

    /// 统计。
    pub fn count(&self) -> Result<usize, RegistryError> {
        let regs = self
            .registrations
            .read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(regs.len())
    }

    /// 是否为空。
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
