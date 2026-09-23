//! Ed25519 签名验证测试。

use ed25519_dalek::{Signer, SigningKey};

use aicore_contracts::plugin::signature::{PluginSignature, SignatureAlgorithm};
use aicore_plugin::{SignatureVerifier, SignatureVerifyError, TrustEntry, TrustStore};

fn keypair() -> (SigningKey, [u8; 32]) {
    use rand::RngCore;
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    let pk = sk.verifying_key().to_bytes();
    (sk, pk)
}

#[test]
fn sha256_still_works() {
    let sig = SignatureVerifier::compute_sha256_from_bytes(b"hello");
    assert_eq!(sig.algorithm, SignatureAlgorithm::Sha256);
    assert!(sig.is_well_formed());
    assert!(SignatureVerifier::verify_bytes(b"hello", &sig).is_ok());
}

#[test]
fn ed25519_valid_signature_passes() {
    let (sk, pk) = keypair();
    let message = b"plugin binary content v1";
    let signature = sk.sign(message);

    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk));
    assert!(sig.is_well_formed());

    assert!(SignatureVerifier::verify_bytes(message, &sig).is_ok());
}

#[test]
fn ed25519_wrong_message_fails() {
    let (sk, pk) = keypair();
    let signature = sk.sign(b"original message");
    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk));

    let err = SignatureVerifier::verify_bytes(b"tampered message", &sig).unwrap_err();
    assert!(matches!(err, SignatureVerifyError::CryptoMismatch));
}

#[test]
fn ed25519_wrong_key_fails() {
    let (sk1, _) = keypair();
    let (_, pk2) = keypair();
    let message = b"data";
    let signature = sk1.sign(message);

    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk2));
    let err = SignatureVerifier::verify_bytes(message, &sig).unwrap_err();
    assert!(matches!(err, SignatureVerifyError::CryptoMismatch));
}

#[test]
fn ed25519_missing_public_key_fails() {
    let mut sig = PluginSignature::ed25519("00".repeat(64), "00".repeat(32));
    sig.public_key_hex = None;

    let err = SignatureVerifier::verify_bytes(b"data", &sig).unwrap_err();
    assert!(matches!(err, SignatureVerifyError::MalformedExpected));
}

#[test]
fn ed25519_malformed_signature_length_fails() {
    let (_, pk) = keypair();
    let sig = PluginSignature::ed25519("00".repeat(10), hex::encode(pk));
    // well-formed 检查会拒
    assert!(!sig.is_well_formed());
}

#[test]
fn ed25519_public_key_mismatch_detected_at_trust_level() {
    let (sk, pk) = keypair();
    let (_, other_pk) = keypair();
    let message = b"trusted plugin artifact";
    let signature = sk.sign(message);
    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk));

    let store = TrustStore::new();
    store
        .register(TrustEntry::trusted("plugin.test", hex::encode(other_pk)))
        .unwrap();
    let result = store.verify("plugin.test", message, Some(&sig)).unwrap();
    assert!(!result.accepted);
    assert!(result.reason.contains("public key mismatch"));
}
