use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::plugin::{
    manifest::TrustLevel, permission::PluginPermission, signature::PluginSignature,
};

use crate::signature::{SignatureVerifier, SignatureVerifyError};

/// Trust Store entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustEntry {
    pub plugin_id: String,
    pub level: TrustLevel,
    /// The desired Ed25519 public key (hex); Trusted must come, other level can be None.
    pub public_key_hex: Option<String>,
    /// Whether to allow a control class.
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

/// Verify results.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustVerification {
    pub plugin_id: String,
    pub accepted: bool,
    pub level: TrustLevel,
    pub reason: String,
}

/// Trust Store。
///
/// Duties:
/// - Record plugin_id confidence and public key.
/// - Verify whether the signature is from a trusted public key.
/// - Rejects the control permission for untrusted plugins.
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

    /// Could not close temporary folder: %s
    ///
    /// Semantics:
    /// - BuiltIn/LocalDev: Skip signature verification; return the corresponding level.
    /// - Trusted: Ed25519 is required to sign and match the public key; does not match the rejection.
    /// - Untrusted: Rejected.
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

                // Trusted must bind the Ed25519 public key in TrustStore.
                if sig.algorithm != aicore_contracts::plugin::signature::SignatureAlgorithm::Ed25519
                {
                    return Ok(TrustVerification {
                        plugin_id: plugin_id.to_string(),
                        accepted: false,
                        level: entry.level,
                        reason: "trusted plugin requires Ed25519 signature".to_string(),
                    });
                }
                let expected_pk = match &entry.public_key_hex {
                    Some(key) if !key.trim().is_empty() => key,
                    _ => {
                        return Ok(TrustVerification {
                            plugin_id: plugin_id.to_string(),
                            accepted: false,
                            level: entry.level,
                            reason: "trusted plugin has no pinned public key".to_string(),
                        });
                    },
                };
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

    /// Checks if plugins have control privileges.
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
