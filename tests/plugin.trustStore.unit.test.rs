//! Trust Store 测试。

use ed25519_dalek::{Signer, SigningKey};

use aicore_contracts::plugin::{
    manifest::TrustLevel, permission::PluginPermission, signature::PluginSignature,
};
use aicore_plugin::{TrustEntry, TrustStore};

fn keypair() -> (SigningKey, [u8; 32]) {
    use rand::RngCore;
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    let pk = sk.verifying_key().to_bytes();
    (sk, pk)
}

#[test]
fn empty_store_rejects_unknown_plugin() {
    let store = TrustStore::new();
    let v = store.verify("unknown", b"data", None).unwrap();
    assert!(!v.accepted);
    assert_eq!(v.level, TrustLevel::Untrusted);
}

#[test]
fn built_in_plugin_skips_signature() {
    let store = TrustStore::new();
    store
        .register(TrustEntry::built_in("core.adapter"))
        .unwrap();

    let v = store.verify("core.adapter", b"data", None).unwrap();
    assert!(v.accepted);
    assert_eq!(v.level, TrustLevel::BuiltIn);
}

#[test]
fn trusted_plugin_requires_signature() {
    let (_, pk) = keypair();
    let store = TrustStore::new();
    store
        .register(TrustEntry::trusted("vendor.gpu", hex::encode(pk)))
        .unwrap();

    let v = store.verify("vendor.gpu", b"data", None).unwrap();
    assert!(!v.accepted);
    assert!(v.reason.contains("requires signature"));
}

#[test]
fn trusted_plugin_with_valid_signature_accepted() {
    let (sk, pk) = keypair();
    let store = TrustStore::new();
    store
        .register(TrustEntry::trusted("vendor.gpu", hex::encode(pk)))
        .unwrap();

    let message = b"plugin binary";
    let signature = sk.sign(message);
    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk));

    let v = store.verify("vendor.gpu", message, Some(&sig)).unwrap();
    assert!(v.accepted, "reason: {}", v.reason);
    assert_eq!(v.level, TrustLevel::Trusted);
}

#[test]
fn trusted_plugin_with_wrong_key_rejected() {
    let (sk, _) = keypair();
    let (_, pk_other) = keypair();
    let store = TrustStore::new();
    store
        .register(TrustEntry::trusted("vendor.gpu", hex::encode(pk_other)))
        .unwrap();

    let message = b"data";
    let signature = sk.sign(message);
    let sig = PluginSignature::ed25519(
        hex::encode(signature.to_bytes()),
        hex::encode(sk.verifying_key().to_bytes()),
    );

    let v = store.verify("vendor.gpu", message, Some(&sig)).unwrap();
    assert!(!v.accepted);
    assert!(v.reason.contains("public key mismatch"));
}

#[test]
fn untrusted_plugin_rejected_even_with_signature() {
    let (sk, pk) = keypair();
    let store = TrustStore::new();
    store
        .register(TrustEntry::untrusted("random.plugin"))
        .unwrap();

    let message = b"data";
    let signature = sk.sign(message);
    let sig = PluginSignature::ed25519(hex::encode(signature.to_bytes()), hex::encode(pk));

    let v = store.verify("random.plugin", message, Some(&sig)).unwrap();
    assert!(!v.accepted);
}

#[test]
fn permits_control_semantics() {
    let store = TrustStore::new();
    store.register(TrustEntry::built_in("b")).unwrap();
    store.register(TrustEntry::local_dev("d")).unwrap();
    store.register(TrustEntry::untrusted("u")).unwrap();

    // read 权限永远允许
    assert!(store
        .permits_control("u", &[PluginPermission::TelemetryRead])
        .unwrap());

    // control 权限：built-in 允许，local-dev / untrusted 拒绝
    assert!(store
        .permits_control("b", &[PluginPermission::ProcessControl])
        .unwrap());
    assert!(!store
        .permits_control("d", &[PluginPermission::ProcessControl])
        .unwrap());
    assert!(!store
        .permits_control("u", &[PluginPermission::ProcessControl])
        .unwrap());
}

#[test]
fn register_rejects_empty_id() {
    let store = TrustStore::new();
    let e = TrustEntry::built_in("");
    assert!(store.register(e).is_err());
}

#[test]
fn remove_and_count() {
    let store = TrustStore::new();
    store.register(TrustEntry::built_in("a")).unwrap();
    store.register(TrustEntry::built_in("b")).unwrap();
    assert_eq!(store.count().unwrap(), 2);
    assert!(store.remove("a").unwrap());
    assert_eq!(store.count().unwrap(), 1);
    assert!(!store.remove("a").unwrap());
}
