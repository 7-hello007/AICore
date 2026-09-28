use serde::{Deserialize, Serialize};

/// Signing algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureAlgorithm {
    /// SHA-256 Hash (V3-K; only for completeness verification, not encrypted signature).
    Sha256,
    /// Ed25519 Public Key Signature (V3.2 Phase 7).
    Ed25519,
}

impl SignatureAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Ed25519 => "ed25519",
        }
    }

    /// Whether or not the signature is encrypted (credible source).
    pub fn is_cryptographic(self) -> bool {
        matches!(self, Self::Ed25519)
    }
}

/// Plugin signature.
///
/// - SHA-256: `hash_hex` is a file Hash; `public_key_hex` is None.
/// - Ed25519: `hash_hex` is signed (64 bytes hex); `public_key_hex` is public key (32 bytes hex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSignature {
    pub algorithm: SignatureAlgorithm,
    pub hash_hex: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key_hex: Option<String>,
}

impl PluginSignature {
    pub fn sha256(hash_hex: impl Into<String>) -> Self {
        Self {
            algorithm: SignatureAlgorithm::Sha256,
            hash_hex: hash_hex.into(),
            public_key_hex: None,
        }
    }

    pub fn ed25519(signature_hex: impl Into<String>, public_key_hex: impl Into<String>) -> Self {
        Self {
            algorithm: SignatureAlgorithm::Ed25519,
            hash_hex: signature_hex.into(),
            public_key_hex: Some(public_key_hex.into()),
        }
    }

    pub fn is_well_formed(&self) -> bool {
        match self.algorithm {
            SignatureAlgorithm::Sha256 => {
                let h = self.hash_hex.trim();
                h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())
            },
            SignatureAlgorithm::Ed25519 => {
                let s = self.hash_hex.trim();
                let s_ok = s.len() == 128 && s.chars().all(|c| c.is_ascii_hexdigit());
                let p_ok = self
                    .public_key_hex
                    .as_ref()
                    .map(|p| {
                        let p = p.trim();
                        p.len() == 64 && p.chars().all(|c| c.is_ascii_hexdigit())
                    })
                    .unwrap_or(false);
                s_ok && p_ok
            },
        }
    }
}
