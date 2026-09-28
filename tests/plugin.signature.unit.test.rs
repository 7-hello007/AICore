//! SignatureVerifier unit test.

use std::path::PathBuf;

use aicore_contracts::plugin::signature::{PluginSignature, SignatureAlgorithm};
use aicore_plugin::{SignatureVerifier, SignatureVerifyError};

fn tmp_file(name: &str, content: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aicore-sig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn compute_sha256_from_bytes() {
    let sig = SignatureVerifier::compute_sha256_from_bytes(b"hello");
    assert_eq!(sig.algorithm, SignatureAlgorithm::Sha256);
    assert!(sig.is_well_formed());
    // Known value of SHA-256 ( "hello")
    assert_eq!(
        sig.hash_hex,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn compute_sha256_from_file() {
    let path = tmp_file("a.bin", b"hello");
    let sig = SignatureVerifier::compute_sha256(&path).unwrap();
    assert_eq!(
        sig.hash_hex,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn compute_empty_input() {
    let sig = SignatureVerifier::compute_sha256_from_bytes(b"");
    // Known value of SHA-256(")
    assert_eq!(
        sig.hash_hex,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn verify_matches() {
    let path = tmp_file("b.bin", b"world");
    let sig = SignatureVerifier::compute_sha256(&path).unwrap();
    assert!(SignatureVerifier::verify_file(&path, &sig).is_ok());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn verify_mismatch() {
    let path = tmp_file("c.bin", b"world");
    let wrong =
        PluginSignature::sha256("0000000000000000000000000000000000000000000000000000000000000000");
    let err = SignatureVerifier::verify_file(&path, &wrong).unwrap_err();
    assert!(matches!(err, SignatureVerifyError::Mismatch { .. }));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn verify_malformed_expected() {
    let path = tmp_file("d.bin", b"world");
    let bad = PluginSignature::sha256("short");
    let err = SignatureVerifier::verify_file(&path, &bad).unwrap_err();
    assert_eq!(err, SignatureVerifyError::MalformedExpected);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn verify_nonexistent_file() {
    let sig =
        PluginSignature::sha256("0000000000000000000000000000000000000000000000000000000000000000");
    let err = SignatureVerifier::verify_file("/nonexistent/file", &sig).unwrap_err();
    assert!(matches!(err, SignatureVerifyError::Io(_)));
}

#[test]
fn signature_well_formed() {
    let good =
        PluginSignature::sha256("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    assert!(good.is_well_formed());

    let bad = PluginSignature::sha256("xyz");
    assert!(!bad.is_well_formed());
}
