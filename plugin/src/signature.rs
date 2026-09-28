use std::io::Read;
use std::path::Path;

use aicore_contracts::plugin::signature::{PluginSignature, SignatureAlgorithm};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Signature certifier.
pub struct SignatureVerifier;

impl SignatureVerifier {
    /// SHA-256 (hexadecimal lowercase) for calculating files.
    pub fn compute_sha256(path: impl AsRef<Path>) -> std::io::Result<PluginSignature> {
        let mut file = std::fs::File::open(path.as_ref())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(PluginSignature::sha256(format!("{:x}", hasher.finalize())))
    }

    /// Calculates SHA-256 from memory bytes.
    pub fn compute_sha256_from_bytes(bytes: &[u8]) -> PluginSignature {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        PluginSignature::sha256(format!("{:x}", digest))
    }

    /// Verify the signature of the document.
    pub fn verify_file(
        path: impl AsRef<Path>,
        expected: &PluginSignature,
    ) -> Result<(), SignatureVerifyError> {
        if !expected.is_well_formed() {
            return Err(SignatureVerifyError::MalformedExpected);
        }
        if expected.algorithm == SignatureAlgorithm::Sha256 {
            let actual = Self::compute_sha256(path.as_ref())
                .map_err(|e| SignatureVerifyError::Io(e.to_string()))?;
            if actual.hash_hex != expected.hash_hex {
                return Err(SignatureVerifyError::Mismatch {
                    expected: expected.hash_hex.clone(),
                    actual: actual.hash_hex,
                });
            }
            return Ok(());
        }
        let bytes =
            std::fs::read(path.as_ref()).map_err(|e| SignatureVerifyError::Io(e.to_string()))?;
        Self::verify_bytes(&bytes, expected)
    }

    /// Verifys the signature of the memory bytes.
    pub fn verify_bytes(
        bytes: &[u8],
        expected: &PluginSignature,
    ) -> Result<(), SignatureVerifyError> {
        if !expected.is_well_formed() {
            return Err(SignatureVerifyError::MalformedExpected);
        }
        match expected.algorithm {
            SignatureAlgorithm::Sha256 => {
                let actual = Self::compute_sha256_from_bytes(bytes);
                if actual.hash_hex != expected.hash_hex {
                    return Err(SignatureVerifyError::Mismatch {
                        expected: expected.hash_hex.clone(),
                        actual: actual.hash_hex,
                    });
                }
                Ok(())
            },
            SignatureAlgorithm::Ed25519 => Self::verify_ed25519(bytes, expected),
        }
    }

    fn verify_ed25519(
        message: &[u8],
        expected: &PluginSignature,
    ) -> Result<(), SignatureVerifyError> {
        let sig_hex = expected.hash_hex.trim();
        let pk_hex = expected
            .public_key_hex
            .as_ref()
            .ok_or(SignatureVerifyError::MissingPublicKey)?
            .trim();

        let sig_bytes = hex::decode(sig_hex)
            .map_err(|e| SignatureVerifyError::MalformedSignature(e.to_string()))?;
        let pk_bytes = hex::decode(pk_hex)
            .map_err(|e| SignatureVerifyError::MalformedPublicKey(e.to_string()))?;

        if sig_bytes.len() != 64 {
            return Err(SignatureVerifyError::MalformedSignature(format!(
                "expected 64 bytes, got {}",
                sig_bytes.len()
            )));
        }
        if pk_bytes.len() != 32 {
            return Err(SignatureVerifyError::MalformedPublicKey(format!(
                "expected 32 bytes, got {}",
                pk_bytes.len()
            )));
        }

        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_arr);

        let mut pk_arr = [0u8; 32];
        pk_arr.copy_from_slice(&pk_bytes);
        let verifying_key = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|e| SignatureVerifyError::MalformedPublicKey(e.to_string()))?;

        verifying_key
            .verify(message, &signature)
            .map_err(|_| SignatureVerifyError::CryptoMismatch)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SignatureVerifyError {
    #[error("unsupported signature algorithm")]
    UnsupportedAlgorithm,
    #[error("expected signature is malformed")]
    MalformedExpected,
    #[error("io error: {0}")]
    Io(String),
    #[error("hash mismatch: expected {expected}, actual {actual}")]
    Mismatch { expected: String, actual: String },
    #[error("missing public key for Ed25519")]
    MissingPublicKey,
    #[error("malformed signature: {0}")]
    MalformedSignature(String),
    #[error("malformed public key: {0}")]
    MalformedPublicKey(String),
    #[error("ed25519 signature verification failed")]
    CryptoMismatch,
}
