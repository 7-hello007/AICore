use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::plugin::{
    manifest::TrustLevel, permission::PluginPermission, signature::PluginSignature,
};

use crate::signature::{SignatureVerifier, SignatureVerifyError};

/// Trust Store 条目。
#[derive(Debug, Clone, PartialEq)]
pub struct TrustEntry {
    pub plugin_id: String,
    pub level: TrustLevel,
    /// 期望的 Ed25519 公钥（hex）；None 表示只按 level 判定。
    pub public_key_hex: Option<String>,
    /// 是否允许 control 类权限。
    pub allow_control: bool,
}

impl TrustEntry {
    pub fn trusted(plugin_id: impl Into<String>, public_key_hex: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            level: TrustLevel::Trusted,
            public_key_hex: Some(public_key_hex.into()),
            allow_control: true,
        }
    }

    pub fn built_in(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            level: TrustLevel::BuiltIn,
            public_key_hex: None,
            allow_control: true,
        }
    }

    pub fn local_dev(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            level: TrustLevel::LocalDev,
            public_key_hex: None,
            allow_control: false,
        }
    }

    pub fn untrusted(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            level: TrustLevel::Untrusted,
            public_key_hex: None,
            allow_control: false,
        }
    }
}

/// 验证结果。
#[derive(Debug, Clone, PartialEq)]
pub struct TrustVerification {
    pub plugin_id: String,
    pub accepted: bool,
    pub level: TrustLevel,
    pub reason: String,
}

/// Trust Store。
///
/// 职责：
/// - 记录 plugin_id 的信任等级与公钥。
/// - 验证签名是否来自已信任的公钥。
/// - 拒绝未信任插件的 control 权限。
pub struct TrustStore {
    entries: RwLock<HashMap<String, TrustEntry>>,
}

impl TrustStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, entry: TrustEntry) -> Result<(), TrustError> {
        if entry.plugin_id.trim().is_empty() {
            return Err(TrustError::InvalidId);
        }
        let mut entries = self.entries.write().map_err(|_| TrustError::LockPoisoned)?;
        entries.insert(entry.plugin_id.clone(), entry);
        Ok(())
    }

    pub fn remove(&self, plugin_id: &str) -> Result<bool, TrustError> {
        let mut entries = self.entries.write().map_err(|_| TrustError::LockPoisoned)?;
        Ok(entries.remove(plugin_id).is_some())
    }

    pub fn get(&self, plugin_id: &str) -> Result<Option<TrustEntry>, TrustError> {
        let entries = self.entries.read().map_err(|_| TrustError::LockPoisoned)?;
        Ok(entries.get(plugin_id).cloned())
    }

    pub fn count(&self) -> Result<usize, TrustError> {
        let entries = self.entries.read().map_err(|_| TrustError::LockPoisoned)?;
        Ok(entries.len())
    }

    /// 验证插件签名是否来自已信任的公钥。
    ///
    /// 语义：
    /// - BuiltIn / LocalDev：跳过签名校验；返回对应 level。
    /// - Trusted：要求 Ed25519 签名且公钥匹配；不匹配 → 拒绝。
    /// - Untrusted：拒绝。
    pub fn verify(
        &self,
        plugin_id: &str,
        artifact_bytes: &[u8],
        signature: Option<&PluginSignature>,
    ) -> Result<TrustVerification, TrustError> {
        let entry = match self.get(plugin_id)? {
            Some(e) => e,
            None => {
                return Ok(TrustVerification {
                    plugin_id: plugin_id.to_string(),
                    accepted: false,
                    level: TrustLevel::Untrusted,
                    reason: "plugin not registered in trust store".to_string(),
                });
            },
        };

        match entry.level {
            TrustLevel::BuiltIn | TrustLevel::LocalDev => Ok(TrustVerification {
                plugin_id: plugin_id.to_string(),
                accepted: true,
                level: entry.level,
                reason: format!("{} plugin: signature check skipped", entry.level_as_str()),
            }),
            TrustLevel::Untrusted => Ok(TrustVerification {
                plugin_id: plugin_id.to_string(),
                accepted: false,
                level: TrustLevel::Untrusted,
                reason: "untrusted plugin rejected".to_string(),
            }),
            TrustLevel::Trusted => {
                let sig = match signature {
                    Some(s) => s,
                    None => {
                        return Ok(TrustVerification {
                            plugin_id: plugin_id.to_string(),
                            accepted: false,
                            level: entry.level,
                            reason: "trusted plugin requires signature".to_string(),
                        });
                    },
                };

                // 校验公钥是否匹配
                if let Some(expected_pk) = &entry.public_key_hex {
                    let actual_pk = sig.public_key_hex.as_ref().ok_or(TrustError::Signature(
                        SignatureVerifyError::MissingPublicKey,
                    ))?;
                    if actual_pk != expected_pk {
                        return Ok(TrustVerification {
                            plugin_id: plugin_id.to_string(),
                            accepted: false,
                            level: entry.level,
                            reason: "public key mismatch".to_string(),
                        });
                    }
                }

                match SignatureVerifier::verify_bytes(artifact_bytes, sig) {
                    Ok(()) => Ok(TrustVerification {
                        plugin_id: plugin_id.to_string(),
                        accepted: true,
                        level: entry.level,
                        reason: "signature verified".to_string(),
                    }),
                    Err(e) => Ok(TrustVerification {
                        plugin_id: plugin_id.to_string(),
                        accepted: false,
                        level: entry.level,
                        reason: format!("signature verification failed: {}", e),
                    }),
                }
            },
        }
    }

    /// 检查插件是否有 control 权限。
    pub fn permits_control(
        &self,
        plugin_id: &str,
        requested: &[PluginPermission],
    ) -> Result<bool, TrustError> {
        let entry = match self.get(plugin_id)? {
            Some(e) => e,
            None => return Ok(false),
        };
        let wants_control = requested.iter().any(|p| p.is_control());
        if !wants_control {
            return Ok(true);
        }
        Ok(entry.allow_control)
    }
}

impl Default for TrustStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TrustEntry {
    fn level_as_str(&self) -> &'static str {
        match self.level {
            TrustLevel::BuiltIn => "built-in",
            TrustLevel::Trusted => "trusted",
            TrustLevel::LocalDev => "local-dev",
            TrustLevel::Untrusted => "untrusted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TrustError {
    #[error("trust store lock poisoned")]
    LockPoisoned,
    #[error("invalid plugin id")]
    InvalidId,
    #[error("signature error: {0}")]
    Signature(#[from] SignatureVerifyError),
}
